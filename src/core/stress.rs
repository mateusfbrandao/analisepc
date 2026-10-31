use rayon::prelude::*;
use std::env;
use std::fs::{self, OpenOptions};
use std::hint::black_box;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::core::sensors::SensorsCollector;

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub single_core_score: u64,
    pub multi_core_score: u64,
    pub duration_secs: f64,
}

#[derive(Debug, Clone)]
pub struct RamBenchmarkResult {
    pub read_gbps: f64,
    pub write_gbps: f64,
    pub copy_gbps: f64,
    pub latency_ns: f64,
}

#[derive(Debug, Clone)]
pub struct DiskBenchmarkResult {
    pub seq_write_mbps: f64,
    pub seq_read_mbps: f64,
    pub test_size_mib: u64,
    pub target_path: String,
}

pub struct StressEngine;

impl StressEngine {
    /// Executa um cálculo pesado de ponto flutuante e trigonometria
    fn heavy_compute_loop(iterations: u64) -> f64 {
        let mut acc = 1.0f64;
        for i in 1..=iterations {
            let fi = i as f64;
            acc = (acc + fi).sin().cos().abs() + (fi * 0.0001).sqrt();
        }
        black_box(acc)
    }

    /// Roda um benchmark rápido de CPU (Single Core e Multi Core)
    pub fn run_cpu_benchmark(test_duration_millis: u64) -> BenchmarkResult {
        let target_duration = std::time::Duration::from_millis(test_duration_millis);

        // 1. Single-core benchmark
        let start_single = Instant::now();
        let mut single_ops = 0u64;
        while start_single.elapsed() < target_duration {
            Self::heavy_compute_loop(20_000);
            single_ops += 1;
        }
        let single_score = single_ops * 100;

        // 2. Multi-core benchmark
        let num_threads = rayon::current_num_threads();
        let start_multi = Instant::now();
        let multi_ops: u64 = (0..num_threads)
            .into_par_iter()
            .map(|_| {
                let start_thread = Instant::now();
                let mut ops = 0u64;
                while start_thread.elapsed() < target_duration {
                    Self::heavy_compute_loop(20_000);
                    ops += 1;
                }
                ops
            })
            .sum();

        let multi_score = multi_ops * 100;

        BenchmarkResult {
            single_core_score: single_score,
            multi_core_score: multi_score,
            duration_secs: start_single.elapsed().as_secs_f64()
                + start_multi.elapsed().as_secs_f64(),
        }
    }

    /// Mede a largura de banda real da Memória RAM (Leitura, Escrita e Cópia em GB/s) + Latência (ns)
    pub fn run_ram_benchmark() -> RamBenchmarkResult {
        // Buffer de 256 MiB (33.554.432 u64s) para superar largamente o cache L3 da CPU e medir a DDR4/DDR5 real
        let len: usize = 32 * 1024 * 1024;
        let bytes_per_pass = (len * std::mem::size_of::<u64>()) as f64;
        let passes = 12usize;
        let total_gb = (bytes_per_pass * passes as f64) / 1_000_000_000.0;

        let mut buf_a = vec![1u64; len];
        let mut buf_b = vec![2u64; len];

        // 1. Escrita Paralela (Write Bandwidth)
        let start_write = Instant::now();
        for p in 0..passes {
            let seed = (p as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
            buf_a.par_chunks_mut(262_144).for_each(|chunk| {
                for (idx, val) in chunk.iter_mut().enumerate() {
                    *val = black_box(seed.wrapping_add(idx as u64));
                }
            });
            black_box(&buf_a);
        }
        let write_secs = start_write.elapsed().as_secs_f64().max(0.0001);
        let write_gbps = total_gb / write_secs;

        // 2. Leitura Paralela (Read Bandwidth)
        let start_read = Instant::now();
        let mut total_acc = 0u64;
        for _ in 0..passes {
            let sum: u64 = buf_a
                .par_chunks(262_144)
                .map(|chunk| {
                    let mut local = 0u64;
                    for &v in chunk {
                        local = local.wrapping_add(v);
                    }
                    black_box(local)
                })
                .reduce(|| 0u64, |a, b| a.wrapping_add(b));
            total_acc = total_acc.wrapping_add(black_box(sum));
        }
        black_box(total_acc);
        let read_secs = start_read.elapsed().as_secs_f64().max(0.0001);
        let read_gbps = total_gb / read_secs;

        // 3. Cópia em Memória Paralela (Copy Bandwidth — Leitura + Escrita simultâneas)
        let start_copy = Instant::now();
        for _ in 0..passes {
            buf_b
                .par_chunks_mut(262_144)
                .zip(buf_a.par_chunks(262_144))
                .for_each(|(dst, src)| {
                    dst.copy_from_slice(src);
                    black_box(&dst[0]);
                });
        }
        let copy_secs = start_copy.elapsed().as_secs_f64().max(0.0001);
        // Na cópia trafegam 2x os bytes (1 leitura + 1 escrita)
        let copy_gbps = (total_gb * 2.0) / copy_secs;

        // 4. Latência de Acesso Aleatório DRAM (Pointer Chasing em 128 MiB com salto de Cache Line de 64B)
        let cachelines = 2 * 1024 * 1024; // 2M linhas de 64B = 128 MiB (supera largamente o cache L3)
        let stride_u32 = 16usize; // 16 * 4B = 64 bytes (1 Cache Line)
        let mut order: Vec<u32> = (0..cachelines as u32).collect();
        let mut state = 0x1234_5678u32;
        for i in (1..cachelines).rev() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let j = (state as usize) % (i + 1);
            order.swap(i, j);
        }
        let mut ring = vec![0u32; cachelines * stride_u32];
        for i in 0..cachelines {
            let cur_line = order[i] as usize;
            let next_line = order[(i + 1) % cachelines] as usize;
            ring[cur_line * stride_u32] = (next_line * stride_u32) as u32;
        }
        drop(order);

        let steps = 2_000_000u64;
        let mut idx = 0u32;
        let start_lat = Instant::now();
        for _ in 0..steps {
            idx = unsafe { *ring.get_unchecked(idx as usize) };
        }
        black_box(idx);
        let lat_ns = (start_lat.elapsed().as_nanos() as f64) / (steps as f64);

        RamBenchmarkResult {
            read_gbps,
            write_gbps,
            copy_gbps,
            latency_ns: lat_ns,
        }
    }

    /// Mede a taxa real de escrita e leitura sequencial no SSD NVMe/Disco (MB/s)
    pub fn run_disk_benchmark(size_mib: u64) -> Result<DiskBenchmarkResult, String> {
        unsafe extern "C" {
            fn posix_fadvise(fd: i32, offset: i64, len: i64, advice: i32) -> i32;
        }
        const POSIX_FADV_DONTNEED: i32 = 4;

        let base_dir = env::var("HOME")
            .map(|h| PathBuf::from(h).join(".cache"))
            .unwrap_or_else(|_| PathBuf::from("/var/tmp"));
        let _ = fs::create_dir_all(&base_dir);
        let temp_file = base_dir.join("analisepc_nvme_bench.tmp");

        let block_size = 4 * 1024 * 1024; // Blocos de 4 MiB
        let blocks = (size_mib.max(64) as usize * 1024 * 1024) / block_size;
        let total_mib = (blocks * block_size) as f64 / (1024.0 * 1024.0);

        // Preenche bloco de 4 MiB com padrão não-nulo (evita compressão trivial de zeros)
        let mut block = vec![0u8; block_size];
        for (i, b) in block.iter_mut().enumerate() {
            *b = ((i.wrapping_mul(31).wrapping_add(17)) & 0xFF) as u8;
        }

        // 1. Escrita Sequencial com sync_all (força gravação física no NVMe)
        let start_write = Instant::now();
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_file)
                .map_err(|e| format!("Falha ao criar arquivo de teste em {:?}: {}", temp_file, e))?;

            for _ in 0..blocks {
                file.write_all(&block)
                    .map_err(|e| format!("Erro durante escrita de benchmark: {}", e))?;
            }
            file.sync_all()
                .map_err(|e| format!("Erro ao sincronizar escrita no disco: {}", e))?;

            // Remove as páginas recém-gravadas do Page Cache do Linux para medir a leitura física real do NVMe
            unsafe {
                let _ = posix_fadvise(file.as_raw_fd(), 0, 0, POSIX_FADV_DONTNEED);
            }
        }
        let write_secs = start_write.elapsed().as_secs_f64().max(0.0001);
        let seq_write_mbps = total_mib / write_secs;

        // 2. Leitura Sequencial Física do NVMe
        let mut read_buf = vec![0u8; block_size];
        let start_read = Instant::now();
        {
            let mut file = OpenOptions::new()
                .read(true)
                .open(&temp_file)
                .map_err(|e| format!("Falha ao abrir arquivo para leitura: {}", e))?;
            unsafe {
                let _ = posix_fadvise(file.as_raw_fd(), 0, 0, POSIX_FADV_DONTNEED);
            }
            let _ = file.seek(SeekFrom::Start(0));

            for _ in 0..blocks {
                file.read_exact(&mut read_buf)
                    .map_err(|e| format!("Erro durante leitura de benchmark: {}", e))?;
                black_box(read_buf[0]);
            }
        }
        let read_secs = start_read.elapsed().as_secs_f64().max(0.0001);
        let seq_read_mbps = total_mib / read_secs;

        // Remove sempre o arquivo temporário
        let _ = fs::remove_file(&temp_file);

        Ok(DiskBenchmarkResult {
            seq_write_mbps,
            seq_read_mbps,
            test_size_mib: total_mib as u64,
            target_path: base_dir.display().to_string(),
        })
    }

    /// Retorna a maior temperatura atual da CPU (Package / Core) em °C para a Trava Térmica
    pub fn read_max_cpu_temp_c() -> f32 {
        let mut sens = SensorsCollector::new();
        let report = sens.refresh();
        report
            .temperatures
            .iter()
            .filter(|t| {
                let l = t.label.to_lowercase();
                l.contains("package") || l.contains("core") || l.contains("cpu") || l.contains("tctl")
            })
            .map(|t| t.temperature_c)
            .fold(0.0f32, f32::max)
    }

    /// Roda um teste contínuo de estresse (FPU + Memória) que pode ser parado a qualquer momento via stop_flag
    pub fn run_continuous_stress(stop_flag: Arc<AtomicBool>) {
        let num_threads = rayon::current_num_threads();
        (0..num_threads).into_par_iter().for_each(|t_idx| {
            let mut local_buf = vec![t_idx as u64 + 1; 65_536];
            let mut iter = 0u64;
            while !stop_flag.load(Ordering::Relaxed) {
                Self::heavy_compute_loop(35_000);
                // Também exercita controlador de cache/RAM durante o stress test
                let pos = (iter as usize * 4099) & (local_buf.len() - 1);
                local_buf[pos] = local_buf[pos].wrapping_mul(6364136223846793005).wrapping_add(1);
                black_box(local_buf[pos]);
                iter = iter.wrapping_add(1);
            }
        });
    }
}
