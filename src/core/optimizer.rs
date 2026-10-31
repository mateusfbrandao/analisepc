use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AutostartApp {
    pub name: String,
    pub exec: String,
    pub file_path: PathBuf,
    pub is_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DiskCleanupSummary {
    pub journal_usage: String,
    pub apt_cache_size: String,
    pub thumbnails_size: String,
    pub shader_cache_size: String,
    pub trash_size: String,
}

#[derive(Debug, Clone)]
pub struct ProcessHogInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub ram_mib: f64,
    pub is_system_critical: bool,
}

#[derive(Debug, Clone)]
pub struct GpuProcessInfo {
    pub pid: u32,
    pub name: String,
    pub engine_type: String,
    pub vram_usage: String,
    pub is_system_critical: bool,
}

#[derive(Debug, Clone)]
pub struct ResourceHogsReport {
    pub top_cpu: Vec<ProcessHogInfo>,
    pub top_ram: Vec<ProcessHogInfo>,
    pub gpu_processes: Vec<GpuProcessInfo>,
}

#[derive(Debug, Clone)]
pub struct HybridGpuThermalInfo {
    pub prime_mode: String,
    pub prime_available: bool,
    pub dgpu_runtime_status: String,
    pub acpi_platform_profile: String,
    pub acpi_choices: String,
    pub cpu_turbo_enabled: bool,
    pub turbo_control_available: bool,
    pub prime_offload_cmd: String,
}

pub struct Optimizer;

impl Optimizer {
    // ===== PERFIL DE ENERGIA & GOVERNADOR =====
    pub fn get_power_profile() -> String {
        if let Ok(output) = Command::new("powerprofilesctl").arg("get").output() {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
        "balanced".to_string()
    }

    pub fn set_power_profile(profile: &str) -> Result<(), String> {
        let status = Command::new("powerprofilesctl")
            .args(["set", profile])
            .status()
            .map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            Err("Falha ao definir perfil com powerprofilesctl".into())
        }
    }

    pub fn get_cpu_governor() -> String {
        fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .unwrap_or_else(|_| "Desconhecido".into())
            .trim()
            .to_string()
    }

    // ===== LIMPEZA DE CACHE DE MEMÓRIA =====
    pub fn clean_memory_cache() -> Result<String, String> {
        // 1. Escreve buffers pendentes para os discos
        let _ = Command::new("sync").status();

        // 2. Tenta limpar pagecache e dentries/inodes via pkexec
        let output = Command::new("pkexec")
            .args(["sh", "-c", "echo 3 > /proc/sys/vm/drop_caches"])
            .output();

        match output {
            Ok(o) if o.status.success() => Ok("Cache de páginas e inodes liberados com sucesso!".into()),
            Ok(_) => Ok("Sincronização de buffers de disco (sync) concluída.".into()),
            Err(e) => Err(format!("Erro ao executar limpeza: {}", e)),
        }
    }

    // ===== TRIM NO SSD =====
    pub fn get_fstrim_info() -> String {
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", "fstrim.timer"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text == "active" {
                return "Ativo (Agendado semanalmente pelo systemd)".into();
            }
        }
        "Manual".into()
    }

    pub fn run_fstrim_now() -> Result<String, String> {
        let output = Command::new("pkexec")
            .args(["fstrim", "-av"])
            .output()
            .map_err(|e| format!("Falha ao executar fstrim: {}", e))?;

        if output.status.success() {
            let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if msg.is_empty() {
                Ok("Comando TRIM executado com sucesso em todas as unidades!".into())
            } else {
                Ok(msg)
            }
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if err.is_empty() { "Operação cancelada ou não autorizada.".into() } else { err })
        }
    }

    // ===== PROGRAMAS DE INICIALIZAÇÃO =====
    pub fn list_autostart_apps() -> Vec<AutostartApp> {
        let mut apps = Vec::new();
        let home = std::env::var("HOME").unwrap_or_default();
        let autostart_dir = Path::new(&home).join(".config/autostart");

        if let Ok(entries) = fs::read_dir(autostart_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let mut name = String::new();
                        let mut exec = String::new();
                        let mut enabled = true;

                        for line in content.lines() {
                            let line = line.trim();
                            if let Some(val) = line.strip_prefix("Name=") {
                                if name.is_empty() {
                                    name = val.to_string();
                                }
                            } else if let Some(val) = line.strip_prefix("Exec=") {
                                if exec.is_empty() {
                                    exec = val.to_string();
                                }
                            } else if line == "X-GNOME-Autostart-enabled=false" || line == "Hidden=true" {
                                enabled = false;
                            }
                        }

                        if !name.is_empty() {
                            apps.push(AutostartApp {
                                name,
                                exec,
                                file_path: path,
                                is_enabled: enabled,
                            });
                        }
                    }
                }
            }
        }

        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }

    pub fn set_autostart_enabled(file_path: &Path, enabled: bool) -> Result<(), String> {
        let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
        let mut new_lines = Vec::new();
        let mut found_gnome_key = false;

        for line in content.lines() {
            if line.starts_with("X-GNOME-Autostart-enabled=") {
                new_lines.push(format!("X-GNOME-Autostart-enabled={}", enabled));
                found_gnome_key = true;
            } else if line.starts_with("Hidden=") {
                new_lines.push(format!("Hidden={}", !enabled));
            } else {
                new_lines.push(line.to_string());
            }
        }

        if !found_gnome_key {
            new_lines.push(format!("X-GNOME-Autostart-enabled={}", enabled));
        }

        fs::write(file_path, new_lines.join("\n")).map_err(|e| e.to_string())?;
        Ok(())
    }

    // ===== LIMPEZA INTELIGENTE DE ESPAÇO EM DISCO (UBUNTU / LINUX) =====
    fn dir_size_bytes(path: &Path) -> u64 {
        let mut total = 0u64;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(meta) = fs::symlink_metadata(&p) {
                    if meta.is_file() {
                        total = total.saturating_add(meta.len());
                    } else if meta.is_dir() {
                        total = total.saturating_add(Self::dir_size_bytes(&p));
                    }
                }
            }
        }
        total
    }

    fn clear_dir_contents(path: &Path) -> u64 {
        let mut removed_bytes = 0u64;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(meta) = fs::symlink_metadata(&p) {
                    if meta.is_file() || meta.file_type().is_symlink() {
                        let len = meta.len();
                        if fs::remove_file(&p).is_ok() {
                            removed_bytes = removed_bytes.saturating_add(len);
                        }
                    } else if meta.is_dir() {
                        let sub = Self::dir_size_bytes(&p);
                        if fs::remove_dir_all(&p).is_ok() {
                            removed_bytes = removed_bytes.saturating_add(sub);
                        }
                    }
                }
            }
        }
        removed_bytes
    }

    pub fn format_bytes(bytes: u64) -> String {
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024 * 1024 {
            format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
        } else if bytes >= 1024 {
            format!("{:.0} KiB", bytes as f64 / 1024.0)
        } else {
            "0 MiB (Limpo)".to_string()
        }
    }

    pub fn scan_disk_cleanup() -> DiskCleanupSummary {
        let home = std::env::var("HOME").unwrap_or_default();
        let home_path = Path::new(&home);

        // 1. Logs do Systemd (journalctl --disk-usage)
        let mut journal_usage = "N/D".to_string();
        if let Ok(out) = Command::new("journalctl").arg("--disk-usage").output() {
            if let Ok(txt) = String::from_utf8(out.stdout) {
                // Ex: "Archived and active journals take up 424.0M in the file system."
                if let Some(pos) = txt.find("take up ") {
                    let after = &txt[pos + 8..];
                    if let Some(size_tok) = after.split_whitespace().next() {
                        journal_usage = size_tok.to_string();
                    }
                }
            }
        }

        // 2. Cache do APT (/var/cache/apt/archives/*.deb)
        let apt_dir = Path::new("/var/cache/apt/archives");
        let mut apt_bytes = 0u64;
        let mut deb_count = 0usize;
        if let Ok(entries) = fs::read_dir(apt_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("deb") {
                    if let Ok(meta) = p.metadata() {
                        apt_bytes = apt_bytes.saturating_add(meta.len());
                        deb_count += 1;
                    }
                }
            }
        }
        let apt_cache_size = if deb_count > 0 {
            format!("{} ({} pacotes .deb)", Self::format_bytes(apt_bytes), deb_count)
        } else {
            "0 MiB (Limpo)".to_string()
        };

        // 3. Cache de Miniaturas do GNOME (~/.cache/thumbnails)
        let thumb_bytes = Self::dir_size_bytes(&home_path.join(".cache/thumbnails"));
        let thumbnails_size = Self::format_bytes(thumb_bytes);

        // 4. Cache de Shaders (NVIDIA GLCache / Optix / Mesa / Vulkan)
        let shader_dirs = [
            home_path.join(".cache/nvidia/GLCache"),
            home_path.join(".cache/nvidia/OptixCache"),
            home_path.join(".cache/mesa_shader_cache"),
            home_path.join(".cache/mesa_shader_cache_db"),
        ];
        let shader_bytes: u64 = shader_dirs.iter().map(|d| Self::dir_size_bytes(d)).sum();
        let shader_cache_size = Self::format_bytes(shader_bytes);

        // 5. Lixeira do Usuário (~/.local/share/Trash)
        let trash_bytes = Self::dir_size_bytes(&home_path.join(".local/share/Trash/files"));
        let trash_size = Self::format_bytes(trash_bytes);

        DiskCleanupSummary {
            journal_usage,
            apt_cache_size,
            thumbnails_size,
            shader_cache_size,
            trash_size,
        }
    }

    pub fn clean_journal_logs() -> Result<String, String> {
        let out = Command::new("pkexec")
            .args(["journalctl", "--vacuum-time=7d"])
            .output()
            .map_err(|e| format!("Falha ao executar journalctl: {}", e))?;

        if out.status.success() {
            let summary = Self::scan_disk_cleanup();
            Ok(format!(
                "Logs antigos removidos! Ocupação atual do journald: {}",
                summary.journal_usage
            ))
        } else {
            Err("Operação cancelada ou não autorizada.".into())
        }
    }

    pub fn clean_apt_cache() -> Result<String, String> {
        let out = Command::new("pkexec")
            .args(["apt-get", "clean"])
            .output()
            .map_err(|e| format!("Falha ao executar apt-get clean: {}", e))?;

        if out.status.success() {
            Ok("Cache de pacotes APT (/var/cache/apt/archives) limpo com sucesso!".into())
        } else {
            Err("Operação cancelada ou não autorizada.".into())
        }
    }

    pub fn clean_thumbnails_cache() -> Result<String, String> {
        let home = std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?;
        let thumb_dir = Path::new(&home).join(".cache/thumbnails");
        let freed = Self::clear_dir_contents(&thumb_dir);
        Ok(format!(
            "Cache de miniaturas do GNOME limpo ({} liberados)!",
            Self::format_bytes(freed)
        ))
    }

    pub fn clean_shader_cache() -> Result<String, String> {
        let home = std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?;
        let home_path = Path::new(&home);
        let shader_dirs = [
            home_path.join(".cache/nvidia/GLCache"),
            home_path.join(".cache/nvidia/OptixCache"),
            home_path.join(".cache/mesa_shader_cache"),
            home_path.join(".cache/mesa_shader_cache_db"),
        ];
        let mut freed = 0u64;
        for d in &shader_dirs {
            freed = freed.saturating_add(Self::clear_dir_contents(d));
        }
        Ok(format!(
            "Cache de Shaders NVIDIA/Mesa limpo ({} liberados)!",
            Self::format_bytes(freed)
        ))
    }

    pub fn empty_user_trash() -> Result<String, String> {
        let home = std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?;
        let trash_files = Path::new(&home).join(".local/share/Trash/files");
        let trash_info = Path::new(&home).join(".local/share/Trash/info");
        let freed = Self::clear_dir_contents(&trash_files)
            .saturating_add(Self::clear_dir_contents(&trash_info));
        Ok(format!(
            "Lixeira esvaziada com sucesso ({} liberados)!",
            Self::format_bytes(freed)
        ))
    }

    // ===== DETECTOR DE VILÕES DE RECURSOS (TOP CPU, RAM E GPU VRAM) =====
    fn is_critical_process(name: &str, pid: u32) -> bool {
        if pid <= 2 || pid == std::process::id() {
            return true;
        }
        let lower = name.to_lowercase();
        matches!(
            lower.as_str(),
            "systemd"
                | "gnome-shell"
                | "xwayland"
                | "pipewire"
                | "wireplumber"
                | "dbus-daemon"
                | "gdm"
                | "gdm-wayland-session"
                | "analisepc"
                | "root"
        )
    }

    fn parse_ps_top(sort_arg: &str, limit: usize) -> Vec<ProcessHogInfo> {
        let mut list = Vec::new();
        if let Ok(out) = Command::new("ps")
            .args(["-eo", "pid=,pcpu=,rss=,comm=", sort_arg])
            .output()
        {
            if let Ok(txt) = String::from_utf8(out.stdout) {
                for line in txt.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 4 {
                        continue;
                    }
                    let pid = parts[0].parse::<u32>().unwrap_or(0);
                    if pid == 0 || pid == std::process::id() {
                        continue;
                    }
                    let cpu_pct = parts[1].replace(',', ".").parse::<f32>().unwrap_or(0.0);
                    let rss_kib = parts[2].parse::<u64>().unwrap_or(0);
                    let ram_mib = rss_kib as f64 / 1024.0;
                    let name = parts[3..].join(" ");
                    if name == "ps" {
                        continue;
                    }
                    let is_crit = Self::is_critical_process(&name, pid);

                    list.push(ProcessHogInfo {
                        pid,
                        name,
                        cpu_pct,
                        ram_mib,
                        is_system_critical: is_crit,
                    });
                    if list.len() >= limit {
                        break;
                    }
                }
            }
        }
        list
    }

    pub fn scan_resource_hogs() -> ResourceHogsReport {
        let top_cpu = Self::parse_ps_top("--sort=-pcpu", 5);
        let top_ram = Self::parse_ps_top("--sort=-rss", 5);
        let mut gpu_processes = Vec::new();

        if let Ok(out) = Command::new("nvidia-smi").output() {
            if let Ok(txt) = String::from_utf8(out.stdout) {
                let mut in_procs = false;
                for line in txt.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("Processes:") {
                        in_procs = true;
                        continue;
                    }
                    if !in_procs || !trimmed.starts_with('|') {
                        continue;
                    }
                    let inner = trimmed.trim_matches('|').trim();
                    let tokens: Vec<&str> = inner.split_whitespace().collect();
                    // Formato típico: 0   N/A  N/A   3145   G   /usr/bin/gnome-shell   41MiB
                    if let Some(type_pos) = tokens
                        .iter()
                        .position(|&t| t == "G" || t == "C" || t == "C+G")
                    {
                        if type_pos >= 1 && type_pos + 2 <= tokens.len() {
                            let pid = tokens[type_pos - 1].parse::<u32>().unwrap_or(0);
                            if pid == 0 {
                                continue;
                            }
                            let raw_type = tokens[type_pos];
                            let engine_type = match raw_type {
                                "C" => "CUDA / Compute".to_string(),
                                "C+G" => "Gráfico + CUDA".to_string(),
                                _ => "Gráfico 3D / Vídeo".to_string(),
                            };
                            let last_tok = tokens[tokens.len() - 1];
                            let (proc_slice, vram_usage) =
                                if last_tok.ends_with("MiB") || last_tok == "N/A" {
                                    (&tokens[type_pos + 1..tokens.len() - 1], last_tok.to_string())
                                } else {
                                    (&tokens[type_pos + 1..], "Ativo".to_string())
                                };

                            let full_cmd = proc_slice.join(" ");
                            let short_name = Path::new(&full_cmd)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or(full_cmd);
                            let is_crit = Self::is_critical_process(&short_name, pid);

                            gpu_processes.push(GpuProcessInfo {
                                pid,
                                name: short_name,
                                engine_type,
                                vram_usage,
                                is_system_critical: is_crit,
                            });
                        }
                    }
                }
            }
        }

        ResourceHogsReport {
            top_cpu,
            top_ram,
            gpu_processes,
        }
    }

    pub fn terminate_process(pid: u32, name: &str) -> Result<String, String> {
        if Self::is_critical_process(name, pid) {
            return Err(format!(
                "O processo '{}' (PID {}) é protegido pelo sistema.",
                name, pid
            ));
        }
        let status = Command::new("kill")
            .args(["-15", &pid.to_string()])
            .status()
            .map_err(|e| format!("Falha ao enviar SIGTERM ao PID {}: {}", pid, e))?;

        if status.success() {
            Ok(format!(
                "Sinal de encerramento enviado para '{}' (PID {})!",
                name, pid
            ))
        } else {
            Err(format!(
                "Não foi possível encerrar '{}' (PID {}) — verifique permissões.",
                name, pid
            ))
        }
    }

    // ===== CONTROLE DE GRÁFICOS HÍBRIDOS (NVIDIA OPTIMUS / PRIME) E PERFIL TÉRMICO =====
    pub fn read_hybrid_gpu_thermal() -> HybridGpuThermalInfo {
        let mut prime_mode = "on-demand".to_string();
        let mut prime_available = false;

        if let Ok(out) = Command::new("prime-select").arg("query").output() {
            if out.status.success() {
                prime_available = true;
                let q = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !q.is_empty() {
                    prime_mode = q;
                }
            }
        } else if let Ok(cfg) = fs::read_to_string("/etc/prime-discrete") {
            let trimmed = cfg.trim();
            if !trimmed.is_empty() {
                prime_mode = trimmed.to_string();
            }
        }

        // Inspeciona o estado de energia runtime D3 da GPU dedicada NVIDIA no barramento PCI
        let mut dgpu_runtime_status = "N/D".to_string();
        if let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") {
            for entry in entries.flatten() {
                let p = entry.path();
                let vendor = fs::read_to_string(p.join("vendor")).unwrap_or_default();
                let class = fs::read_to_string(p.join("class")).unwrap_or_default();
                if vendor.trim() == "0x10de"
                    && (class.trim().starts_with("0x0300") || class.trim().starts_with("0x0302"))
                {
                    let rt = fs::read_to_string(p.join("power/runtime_status"))
                        .unwrap_or_else(|_| "unknown".into())
                        .trim()
                        .to_string();
                    dgpu_runtime_status = match rt.as_str() {
                        "active" => "Ativa (Acordada — P8/P0)".to_string(),
                        "suspended" => "Suspensa (D3cold — Economia 0W)".to_string(),
                        other => other.to_string(),
                    };
                    break;
                }
            }
        }

        let acpi_platform_profile = fs::read_to_string("/sys/firmware/acpi/platform_profile")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "{} (via power-profiles-daemon)",
                    Self::get_power_profile()
                )
            });

        let acpi_choices = fs::read_to_string("/sys/firmware/acpi/platform_profile_choices")
            .ok()
            .map(|s| s.trim().replace(' ', " • "))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "power-saver • balanced • performance".to_string());

        let no_turbo_path = Path::new("/sys/devices/system/cpu/intel_pstate/no_turbo");
        let turbo_control_available = no_turbo_path.exists();
        let cpu_turbo_enabled = fs::read_to_string(no_turbo_path)
            .ok()
            .map(|s| s.trim() == "0")
            .unwrap_or(true);

        HybridGpuThermalInfo {
            prime_mode,
            prime_available,
            dgpu_runtime_status,
            acpi_platform_profile,
            acpi_choices,
            cpu_turbo_enabled,
            turbo_control_available,
            prime_offload_cmd:
                "__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia %command%".to_string(),
        }
    }

    pub fn set_prime_mode(mode: &str) -> Result<String, String> {
        let out = Command::new("pkexec")
            .args(["prime-select", mode])
            .output()
            .map_err(|e| format!("Falha ao executar prime-select: {}", e))?;

        if out.status.success() {
            Ok(format!(
                "Perfil NVIDIA PRIME alterado para '{}'! Encerre a sessão (Logout) ou reinicie para aplicar.",
                mode
            ))
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if err.is_empty() {
                "Operação cancelada ou não autorizada.".into()
            } else {
                err
            })
        }
    }

    pub fn set_intel_turbo_enabled(enabled: bool) -> Result<String, String> {
        let val = if enabled { "0" } else { "1" };
        let cmd = format!("echo {} > /sys/devices/system/cpu/intel_pstate/no_turbo", val);
        let out = Command::new("pkexec")
            .args(["sh", "-c", &cmd])
            .output()
            .map_err(|e| format!("Falha ao alternar Intel Turbo Boost: {}", e))?;

        if out.status.success() {
            if enabled {
                Ok("Intel Turbo Boost ativado (Frequência Máxima de Boost liberada)!".into())
            } else {
                Ok("Intel Turbo Boost desativado (Operação fria e silenciosa em Clock Base)!".into())
            }
        } else {
            Err("Operação cancelada ou não autorizada.".into())
        }
    }
}
