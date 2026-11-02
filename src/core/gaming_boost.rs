use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

pub const GAMING_PACKAGES: &[&str] = &[
    "gamemode",
    "libgamemode0:i386",
    "mangohud",
    "libmangohud:i386",
    "gamescope",
    "mesa-utils",
    "vulkan-tools",
];

pub const SHADER_CACHE_MB: u32 = 12_288;
pub const SHADER_CONF_PATH: &str = "/etc/environment.d/99-analisepc-shader-cache.conf";
pub const SPLIT_LOCK_CONF_PATH: &str = "/etc/sysctl.d/99-analisepc-split-lock.conf";

const INSTALL_PACKAGES_SCRIPT: &str = r#"
set -eu
found=0
for arch in $(dpkg --print-foreign-architectures); do
  if [ "$arch" = i386 ]; then
    found=1
  fi
done
if [ "$found" -eq 0 ]; then
  dpkg --add-architecture i386
fi
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y gamemode libgamemode0:i386 mangohud libmangohud:i386 gamescope mesa-utils vulkan-tools
echo "Pacotes de jogos instalados."
"#;

const APPLY_SHADER_SCRIPT: &str = r#"
set -eu
install -d -m 0755 /etc/environment.d
umask 022
cat > /etc/environment.d/99-analisepc-shader-cache.conf << 'EOF'
MESA_GLSL_CACHE_MAX_SIZE=12288
MESA_SHADER_CACHE_MAX_SIZE=12288
EOF
chmod 644 /etc/environment.d/99-analisepc-shader-cache.conf
echo "Cache de shaders da Mesa definido para 12 GB. Encerre a sessão para aplicar."
"#;

const SPLIT_LOCK_OFF_SCRIPT: &str = r#"
set -eu
sysctl -w kernel.split_lock_mitigate=0 >/dev/null
echo "Mitigação de split lock desligada até o próximo boot."
"#;

const SPLIT_LOCK_ON_SCRIPT: &str = r#"
set -eu
sysctl -w kernel.split_lock_mitigate=1 >/dev/null
echo "Mitigação de split lock religada."
"#;

const PERSIST_SPLIT_LOCK_OFF_SCRIPT: &str = r#"
set -eu
install -d -m 0755 /etc/sysctl.d
umask 022
cat > /etc/sysctl.d/99-analisepc-split-lock.conf << 'EOF'
kernel.split_lock_mitigate = 0
EOF
chmod 644 /etc/sysctl.d/99-analisepc-split-lock.conf
sysctl -w kernel.split_lock_mitigate=0 >/dev/null
echo "Mitigação de split lock desligada e gravada para os próximos boots."
"#;

const PERSIST_SPLIT_LOCK_ON_SCRIPT: &str = r#"
set -eu
install -d -m 0755 /etc/sysctl.d
umask 022
cat > /etc/sysctl.d/99-analisepc-split-lock.conf << 'EOF'
kernel.split_lock_mitigate = 1
EOF
chmod 644 /etc/sysctl.d/99-analisepc-split-lock.conf
sysctl -w kernel.split_lock_mitigate=1 >/dev/null
echo "Mitigação de split lock gravada como ativa para os próximos boots."
"#;

const INSTALL_LACT_SCRIPT: &str = r#"
set -eu
if ! command -v curl >/dev/null 2>&1; then
  echo "O comando curl não está instalado." >&2
  exit 1
fi
json=$(curl -fsSL --max-time 60 -A AnalisePC https://api.github.com/repos/ilya-zlobintsev/LACT/releases/latest)
url=$(printf '%s\n' "$json" | grep -oE 'https://github.com/ilya-zlobintsev/LACT/releases/download/[^"[:space:]]+amd64\.deb' | head -n 1 || true)
if [ "${#url}" -gt 512 ]; then
  echo "URL do pacote LACT rejeitada." >&2
  exit 1
fi
case "$url" in
  *..*|*[!A-Za-z0-9._~:/+-]*)
    echo "URL do pacote LACT rejeitada." >&2
    exit 1
    ;;
  https://github.com/ilya-zlobintsev/LACT/releases/download/*amd64.deb)
    ;;
  *)
    echo "URL do pacote LACT rejeitada." >&2
    exit 1
    ;;
esac
dir=$(mktemp -d /tmp/analisepc-lact.XXXXXX)
trap 'rm -rf "$dir"' EXIT
tmp="$dir/lact.deb"
curl -fsSL --max-time 180 -A AnalisePC "$url" -o "$tmp"
export DEBIAN_FRONTEND=noninteractive
apt-get install -y "$tmp"
systemctl enable --now lactd
echo "LACT instalado e serviço lactd ativado. A assinatura do pacote não foi verificada."
"#;

#[derive(Debug, Clone)]
pub struct PackageState {
    pub name: &'static str,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct GamingBoostStatus {
    pub in_flatpak: bool,
    pub apt_available: bool,
    pub i386_enabled: bool,
    pub packages: Vec<PackageState>,
    pub glsl_cache_mb: Option<u32>,
    pub shader_cache_mb: Option<u32>,
    pub shader_boost_applied: bool,
    /// `1` penaliza o processo, `0` desliga a penalidade. `None` se o kernel não expõe o controle.
    pub split_lock_mitigate: Option<u8>,
    pub split_lock_persisted: Option<u8>,
    pub lact_installed: bool,
}

impl GamingBoostStatus {
    pub fn packages_complete(&self) -> bool {
        self.apt_available && self.packages.iter().all(|pkg| pkg.installed)
    }

    pub fn missing_package_names(&self) -> Vec<&'static str> {
        self.packages
            .iter()
            .filter(|pkg| !pkg.installed)
            .map(|pkg| pkg.name)
            .collect()
    }

    pub fn shader_cache_label(&self) -> String {
        match (self.glsl_cache_mb, self.shader_cache_mb) {
            (Some(SHADER_CACHE_MB), Some(SHADER_CACHE_MB)) => "12 GB".to_string(),
            (Some(glsl), Some(shader)) => format!("GLSL {glsl} MB, shader {shader} MB"),
            (Some(glsl), None) => format!("só GLSL em {glsl} MB"),
            (None, Some(shader)) => format!("só shader cache em {shader} MB"),
            (None, None) => "padrão do sistema".to_string(),
        }
    }

    pub fn split_lock_label(&self) -> String {
        let base = match self.split_lock_mitigate {
            Some(0) => "mitigação desligada",
            Some(1) => "mitigação ativa",
            _ => "indisponível neste kernel",
        };
        let note = match (self.split_lock_mitigate, self.split_lock_persisted) {
            (Some(runtime), Some(saved)) if runtime == saved => " e gravada para os próximos boots",
            (Some(_), Some(_)) => " (o arquivo persistente difere do valor atual)",
            _ => "",
        };
        format!("{base}{note}")
    }
}

pub struct GamingBoost;

impl GamingBoost {
    pub fn read() -> GamingBoostStatus {
        let in_flatpak = Self::in_flatpak();
        let apt_available = !in_flatpak && command_exists("apt-get") && command_exists("dpkg");
        let i386_enabled = apt_available && i386_is_enabled();
        let installed = if apt_available {
            installed_packages()
        } else {
            Vec::new()
        };
        let packages = GAMING_PACKAGES
            .iter()
            .copied()
            .map(|name| PackageState {
                name,
                installed: installed.iter().any(|pkg| pkg == name),
            })
            .collect();

        let (glsl_cache_mb, shader_cache_mb) = read_shader_cache_limits();
        let split_lock_mitigate = read_split_lock_runtime();
        let split_lock_persisted = read_split_lock_persisted();

        GamingBoostStatus {
            in_flatpak,
            apt_available,
            i386_enabled,
            packages,
            glsl_cache_mb,
            shader_cache_mb,
            shader_boost_applied: glsl_cache_mb == Some(SHADER_CACHE_MB)
                && shader_cache_mb == Some(SHADER_CACHE_MB),
            split_lock_mitigate,
            split_lock_persisted,
            lact_installed: !in_flatpak && command_exists("lact"),
        }
    }

    pub fn in_flatpak() -> bool {
        Path::new("/.flatpak-info").exists() || std::env::var_os("FLATPAK_ID").is_some()
    }

    pub fn install_gaming_packages() -> Result<String, String> {
        ensure_host_actions()?;
        if !command_exists("apt-get") || !command_exists("dpkg") {
            return Err(
                "A instalação de pacotes só está disponível em Debian, Ubuntu e derivadas.".into(),
            );
        }
        pkexec_static(INSTALL_PACKAGES_SCRIPT)
    }

    pub fn apply_shader_cache() -> Result<String, String> {
        ensure_host_actions()?;
        pkexec_static(APPLY_SHADER_SCRIPT)
    }

    pub fn set_split_lock_mitigate(penalize: bool) -> Result<String, String> {
        ensure_host_actions()?;
        if penalize {
            pkexec_static(SPLIT_LOCK_ON_SCRIPT)
        } else {
            pkexec_static(SPLIT_LOCK_OFF_SCRIPT)
        }
    }

    pub fn persist_split_lock(penalize: bool) -> Result<String, String> {
        ensure_host_actions()?;
        if penalize {
            pkexec_static(PERSIST_SPLIT_LOCK_ON_SCRIPT)
        } else {
            pkexec_static(PERSIST_SPLIT_LOCK_OFF_SCRIPT)
        }
    }

    pub fn open_lact() -> Result<String, String> {
        ensure_host_actions()?;
        if !command_exists("lact") {
            return Err("O LACT não está instalado.".into());
        }
        let mut child = Command::new("lact")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("Não foi possível abrir o LACT: {err}"))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok("LACT aberto.".into())
    }

    pub fn install_lact() -> Result<String, String> {
        ensure_host_actions()?;
        if !command_exists("apt-get") {
            return Err("A instalação do LACT por pacote .deb exige apt.".into());
        }
        pkexec_static(INSTALL_LACT_SCRIPT)
    }
}

fn ensure_host_actions() -> Result<(), String> {
    if GamingBoost::in_flatpak() {
        Err("Pacotes, ambiente do sistema, sysctl e serviços só funcionam na instalação .deb, fora do Flatpak.".into())
    } else {
        Ok(())
    }
}

fn pkexec_static(script: &'static str) -> Result<String, String> {
    let output = Command::new("pkexec")
        .args(["sh", "-c", script])
        .output()
        .map_err(|err| format!("Falha ao executar pkexec: {err}"))?;
    if output.status.success() {
        Ok(last_feedback_line(&output.stdout, "Concluído."))
    } else {
        Err(command_failure(&output))
    }
}

fn command_failure(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if raw.is_empty() {
        "Operação cancelada ou não autorizada.".to_string()
    } else {
        sanitize_feedback(raw)
    }
}

fn last_feedback_line(stdout: &[u8], fallback: &str) -> String {
    let text = String::from_utf8_lossy(stdout);
    let line = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(fallback);
    sanitize_feedback(line)
}

fn sanitize_feedback(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|ch| {
            if ch.is_control() && ch != '\n' && ch != '\t' {
                ' '
            } else {
                ch
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    const MAX: usize = 360;
    let count = trimmed.chars().count();
    if count <= MAX {
        trimmed.to_string()
    } else {
        trimmed.chars().skip(count - MAX).collect()
    }
}

fn command_exists(name: &str) -> bool {
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return false;
    }
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    path.split(':')
        .any(|dir| Path::new(dir).join(name).is_file())
}

fn is_safe_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 64
        && name.chars().filter(|ch| *ch == ':').count() <= 1
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.' | ':'))
}

fn i386_is_enabled() -> bool {
    Command::new("dpkg")
        .arg("--print-foreign-architectures")
        .output()
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.trim() == "i386")
        })
        .unwrap_or(false)
}

fn installed_packages() -> Vec<String> {
    let output = Command::new("dpkg-query")
        .args([
            "-W",
            "-f",
            "${Package}\t${Architecture}\t${db:Status-Status}\n",
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.split('\t');
        let Some(name) = parts.next() else { continue };
        let Some(arch) = parts.next() else { continue };
        let Some(status) = parts.next() else { continue };
        if status.trim() != "installed" || !is_safe_package_name(name) {
            continue;
        }
        let qualified = format!("{name}:{arch}");
        if GAMING_PACKAGES.contains(&name) {
            found.push(name.to_string());
        }
        if is_safe_package_name(&qualified) && GAMING_PACKAGES.contains(&qualified.as_str()) {
            found.push(qualified);
        }
    }
    found
}

fn read_shader_cache_limits() -> (Option<u32>, Option<u32>) {
    let mut glsl = None;
    let mut shader = None;
    for (key, value) in read_environment_assignments() {
        let parsed = parse_megabytes(&value);
        if key == "MESA_GLSL_CACHE_MAX_SIZE" {
            glsl = parsed;
        } else if key == "MESA_SHADER_CACHE_MAX_SIZE" {
            shader = parsed;
        }
    }
    (glsl, shader)
}

fn read_environment_assignments() -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    if let Ok(text) = fs::read_to_string("/etc/environment") {
        pairs.extend(parse_env_text(&text));
    }
    let Ok(entries) = fs::read_dir("/etc/environment.d") else {
        return pairs;
    };
    let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|ext| ext.to_str()) != Some("conf") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            pairs.extend(parse_env_text(&text));
        }
    }
    pairs
}

fn parse_env_text(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key
            .trim()
            .strip_prefix("export ")
            .unwrap_or(key.trim())
            .trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        pairs.push((key.to_string(), value.to_string()));
    }
    pairs
}

fn parse_megabytes(value: &str) -> Option<u32> {
    if value.is_empty() || value.len() > 8 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn read_split_lock_runtime() -> Option<u8> {
    let text = fs::read_to_string("/proc/sys/kernel/split_lock_mitigate").ok()?;
    match text.trim() {
        "0" => Some(0),
        "1" => Some(1),
        _ => None,
    }
}

fn read_split_lock_persisted() -> Option<u8> {
    let text = fs::read_to_string(SPLIT_LOCK_CONF_PATH).ok()?;
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let normalized: String = line.chars().filter(|ch| !ch.is_whitespace()).collect();
        found = match normalized.as_str() {
            "kernel.split_lock_mitigate=0" => Some(0),
            "kernel.split_lock_mitigate=1" => Some(1),
            _ => found,
        };
    }
    found
}
