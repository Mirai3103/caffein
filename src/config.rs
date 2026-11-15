use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Duration {
    Infinite,
    M15,
    M30,
    H1,
    H2,
}

pub const ALL: [Duration; 5] = [
    Duration::Infinite,
    Duration::M15,
    Duration::M30,
    Duration::H1,
    Duration::H2,
];

impl Duration {
    pub fn minutes(self) -> Option<u32> {
        match self {
            Duration::Infinite => None,
            Duration::M15 => Some(15),
            Duration::M30 => Some(30),
            Duration::H1 => Some(60),
            Duration::H2 => Some(120),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Duration::Infinite => "Vô hạn",
            Duration::M15 => "15 phút",
            Duration::M30 => "30 phút",
            Duration::H1 => "1 giờ",
            Duration::H2 => "2 giờ",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Duration::Infinite => "inf",
            Duration::M15 => "15",
            Duration::M30 => "30",
            Duration::H1 => "60",
            Duration::H2 => "120",
        }
    }

    pub fn from_key(s: &str) -> Option<Duration> {
        ALL.into_iter().find(|d| d.key() == s)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Config {
    pub autostart: bool,
    pub activate_on_launch: bool,
    pub keep_display: bool,
    pub duration: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            autostart: false,
            activate_on_launch: false,
            keep_display: true,
            duration: Duration::Infinite,
        }
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

impl Config {
    pub fn parse(text: &str) -> Config {
        let mut cfg = Config::default();
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with(';')
                || line.starts_with('[')
            {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let k = k.trim();
            let v = v.trim();
            match k {
                "autostart" => {
                    if let Some(b) = parse_bool(v) {
                        cfg.autostart = b;
                    }
                }
                "activate_on_launch" => {
                    if let Some(b) = parse_bool(v) {
                        cfg.activate_on_launch = b;
                    }
                }
                "keep_display" => {
                    if let Some(b) = parse_bool(v) {
                        cfg.keep_display = b;
                    }
                }
                "duration" => {
                    if let Some(d) = Duration::from_key(v) {
                        cfg.duration = d;
                    }
                }
                _ => {}
            }
        }
        cfg
    }

    pub fn serialize(&self) -> String {
        format!(
            "autostart={}\nactivate_on_launch={}\nkeep_display={}\nduration={}\n",
            u8::from(self.autostart),
            u8::from(self.activate_on_launch),
            u8::from(self.keep_display),
            self.duration.key()
        )
    }

    pub fn path() -> Option<PathBuf> {
        let appdata = std::env::var_os("APPDATA")?;
        if appdata.is_empty() {
            return None;
        }
        Some(PathBuf::from(appdata).join("Caffein").join("config.ini"))
    }

    pub fn load() -> Config {
        let Some(path) = Self::path() else {
            return Config::default();
        };
        match std::fs::read_to_string(path) {
            Ok(text) => Config::parse(&text),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA is not set")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let c = Config::default();
        assert_eq!(
            (c.autostart, c.activate_on_launch, c.keep_display, c.duration),
            (false, false, true, Duration::Infinite)
        );
    }

    #[test]
    fn roundtrip() {
        let c = Config {
            autostart: true,
            activate_on_launch: true,
            keep_display: false,
            duration: Duration::H1,
        };
        assert_eq!(Config::parse(&c.serialize()), c);
    }

    #[test]
    fn garbage_falls_back_to_default() {
        let c = Config::parse("[x]\n;cmt\nduration=abc\nkeep_display=maybe\nfoo=bar\n=\n\u{feff}");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn partial_and_whitespace() {
        let c = Config::parse("  duration = 30 \r\nautostart=1");
        assert_eq!(c.duration, Duration::M30);
        assert!(c.autostart);
        assert!(c.keep_display); // giữ mặc định
    }

    #[test]
    fn duration_keys() {
        for d in ALL {
            assert_eq!(Duration::from_key(d.key()), Some(d));
        }
        assert_eq!(Duration::H2.minutes(), Some(120));
        assert_eq!(Duration::Infinite.minutes(), None);
    }
}
