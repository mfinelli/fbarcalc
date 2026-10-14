/* fbarcalc: find account maximum value
 * Copyright 2024-2026 Mario Finelli
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

extern crate dirs;

use inquire::error::InquireError;
use inquire::{Password, Select};
use std::error::Error;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;
use toml_edit::DocumentMut;

const CONFIG_VERSION: i64 = 1;

const CONFIG_DEFAULT_INPUT_CURRENCY: &str = "default_input_currency";
const CONFIG_FCA_API_KEY: &str = "fca_api_key";

pub const SUPPORTED_CURRENCIES: [&Currency; 4] = [
    &Currency {
        code: "EUR",
        name: "Euro",
        symbol: "€",
    },
    &Currency {
        code: "GBP",
        name: "British Pound Sterling",
        symbol: "£",
    },
    &Currency {
        code: "JPY",
        name: "Japanese Yen",
        symbol: "¥",
    },
    &Currency {
        code: "USD",
        name: "US Dollar",
        symbol: "$",
    },
];

#[derive(Clone, Debug)]
pub struct Currency {
    pub code: &'static str,
    pub name: &'static str,
    pub symbol: &'static str,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub version: i64,
    pub default_input_currency: Option<String>,
    pub fca_api_key: Option<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    /// We don't know where the config file should be.
    NoConfigDir,
    /// The config file doesn't exist.
    NotFound,
    /// The config file couldn't be read or written.
    Io(std::io::Error),
    /// The config file isn't valid TOML.
    Parse(toml_edit::TomlError),
    /// A setting in the config file has the wrong type.
    WrongType {
        key: &'static str,
        expected: &'static str,
    },
    /// Any of the above, for the config file at the given path.
    File {
        path: PathBuf,
        source: Box<ConfigError>,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoConfigDir => write!(
                f,
                "unable to find the config directory, use --config to choose \
                 a config file"
            ),
            ConfigError::NotFound => {
                write!(f, "no such file, run `fbarcalc config` to create it")
            }
            ConfigError::Io(e) => write!(f, "{}", e),
            ConfigError::Parse(e) => write!(f, "{}", e),
            ConfigError::WrongType { key, expected } => {
                write!(f, "`{}` must be {}", key, expected)
            }
            ConfigError::File { path, source } => {
                write!(f, "{}: {}", path.display(), source)
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            ConfigError::File { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Attach the path of the config file to an error.
fn in_file(path: &Path) -> impl Fn(ConfigError) -> ConfigError + '_ {
    move |e| ConfigError::File {
        path: path.to_path_buf(),
        source: Box::new(e),
    }
}

pub fn get_config_file(
    cli_config: Option<PathBuf>,
) -> Result<PathBuf, ConfigError> {
    config_file_in(cli_config, dirs::config_dir())
}

fn config_file_in(
    cli_config: Option<PathBuf>,
    config_dir: Option<PathBuf>,
) -> Result<PathBuf, ConfigError> {
    match cli_config {
        Some(c) => Ok(c),
        None => {
            let mut config_path = config_dir.ok_or(ConfigError::NoConfigDir)?;
            config_path.push("fbarcalc");
            config_path.push("config.toml");
            Ok(config_path)
        }
    }
}

pub fn get_config(config: Option<PathBuf>) -> Result<Config, ConfigError> {
    let config_file = get_config_file(config)?;

    let toml = read_if_exists(&config_file)?
        .ok_or(ConfigError::NotFound)
        .map_err(in_file(&config_file))?;

    parse_config(&toml).map_err(in_file(&config_file))
}

/// Read the file, or `None` if it doesn't exist.
fn read_if_exists(path: &Path) -> Result<Option<String>, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(toml) => Ok(Some(toml)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(in_file(path)(ConfigError::Io(e))),
    }
}

pub fn parse_config(toml: &str) -> Result<Config, ConfigError> {
    let doc = toml.parse::<DocumentMut>().map_err(ConfigError::Parse)?;

    let version = match doc.get("version") {
        None => CONFIG_VERSION,
        Some(i) => i.as_integer().ok_or(ConfigError::WrongType {
            key: "version",
            expected: "an integer",
        })?,
    };

    let string = |key: &'static str| match doc.get(key) {
        None => Ok(None),
        Some(s) => s.as_str().map(|s| Some(s.to_string())).ok_or(
            ConfigError::WrongType {
                key,
                expected: "a string",
            },
        ),
    };

    Ok(Config {
        version,
        default_input_currency: string(CONFIG_DEFAULT_INPUT_CURRENCY)?,
        fca_api_key: string(CONFIG_FCA_API_KEY)?,
    })
}

/// Make sure that the directory for the config file exists.
fn create_config_directory(config_file: &Path) -> Result<(), std::io::Error> {
    match config_file.parent() {
        Some(p) => std::fs::create_dir_all(p),
        None => Ok(()),
    }
}

/// Write the config file, creating it (readable only by us, because it
/// contains an API key) and its directory if they don't exist yet.
pub fn write_config(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let write = || -> Result<(), std::io::Error> {
        create_config_directory(path)?;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents.as_bytes())
    };

    write().map_err(|e| in_file(path)(ConfigError::Io(e)))
}

/// Add the settings to the existing contents of the config file. Everything
/// else in the file (other settings, comments) is left alone, and the API key
/// is only replaced if there is a new one.
pub fn update_config_doc(
    existing: &str,
    currency_code: &str,
    api_key: Option<&str>,
) -> Result<String, ConfigError> {
    let mut doc = existing
        .parse::<DocumentMut>()
        .map_err(ConfigError::Parse)?;

    doc["version"] = toml_edit::value(CONFIG_VERSION);
    doc[CONFIG_DEFAULT_INPUT_CURRENCY] = toml_edit::value(currency_code);

    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        doc[CONFIG_FCA_API_KEY] = toml_edit::value(key);
    }

    Ok(doc.to_string())
}

/// What to show after the API key prompt: stars for what was typed or, if
/// nothing was typed (which keeps the existing key), stars for the existing
/// key.
fn mask_key(typed: &str, existing: Option<&str>) -> String {
    let shown = if typed.is_empty() {
        existing.unwrap_or_default()
    } else {
        typed
    };

    "*".repeat(shown.chars().count())
}

/// The position in the list of currencies to start the prompt at: the
/// default currency, or the first one if there isn't one or it isn't one that
/// we support.
fn starting_cursor(default: Option<&str>) -> usize {
    default
        .and_then(|code| {
            SUPPORTED_CURRENCIES.iter().position(|c| c.code == code)
        })
        .unwrap_or(0)
}

pub fn select_input_currency(
    default: Option<&str>,
    is_default: bool,
) -> Result<&'static Currency, InquireError> {
    let p = if is_default {
        "What is the default input currency?"
    } else {
        "What is the input currency?"
    };

    Select::new(p, SUPPORTED_CURRENCIES.to_vec())
        .with_starting_cursor(starting_cursor(default))
        .prompt()
}

pub fn configure(config: Option<PathBuf>) -> ExitCode {
    match run_configure(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn run_configure(config: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let config_file = get_config_file(config)?;
    let existing = read_if_exists(&config_file)?.unwrap_or_default();

    let doc = existing
        .parse::<DocumentMut>()
        .map_err(|e| in_file(&config_file)(ConfigError::Parse(e)))?;
    let current_currency = doc
        .get(CONFIG_DEFAULT_INPUT_CURRENCY)
        .and_then(|s| s.as_str());
    let current_key = doc.get(CONFIG_FCA_API_KEY).and_then(|s| s.as_str());

    let currency = select_input_currency(current_currency, true)?;

    let key = Password::new("Freecurrency.com API key:")
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_formatter(&|k| mask_key(k, current_key))
        .without_confirmation()
        .prompt()?;

    save_config(&config_file, &existing, currency.code, &key)?;

    Ok(())
}

/// Update the existing contents of the config file with the answers that we
/// got and write them back.
fn save_config(
    config_file: &Path,
    existing: &str,
    currency_code: &str,
    api_key: &str,
) -> Result<(), ConfigError> {
    let updated = update_config_doc(existing, currency_code, Some(api_key))
        .map_err(in_file(config_file))?;

    write_config(config_file, &updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A directory that is removed when it goes out of scope.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);

            let dir = std::env::temp_dir().join(format!(
                "fbarcalc-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn join(&self, p: &str) -> PathBuf {
            self.0.join(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // currencies

    #[test]
    fn usd_is_a_supported_currency() {
        // the exchange module relies on this code to skip the conversion
        let usd = SUPPORTED_CURRENCIES.iter().find(|c| c.code == "USD");
        assert_eq!(usd.unwrap().symbol, "$");
    }

    #[test]
    fn supported_currencies_are_distinct_and_complete() {
        for (i, a) in SUPPORTED_CURRENCIES.iter().enumerate() {
            assert!(!a.code.is_empty() && !a.name.is_empty());
            assert!(!a.symbol.is_empty());
            for b in &SUPPORTED_CURRENCIES[i + 1..] {
                assert_ne!(a.code, b.code);
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn currencies_are_displayed_by_name() {
        let names: Vec<String> =
            SUPPORTED_CURRENCIES.iter().map(|c| c.to_string()).collect();
        assert_eq!(
            names,
            [
                "Euro",
                "British Pound Sterling",
                "Japanese Yen",
                "US Dollar"
            ]
        );
    }

    #[test]
    fn the_prompt_starts_on_the_default_currency() {
        assert_eq!(starting_cursor(Some("EUR")), 0);
        assert_eq!(starting_cursor(Some("GBP")), 1);
        assert_eq!(starting_cursor(Some("JPY")), 2);
        assert_eq!(starting_cursor(Some("USD")), 3);
    }

    #[test]
    fn the_prompt_starts_on_the_first_currency_without_a_usable_default() {
        assert_eq!(starting_cursor(None), 0);
        assert_eq!(starting_cursor(Some("CAD")), 0);
        assert_eq!(starting_cursor(Some("")), 0);
        assert_eq!(starting_cursor(Some("usd")), 0);
    }

    // config file location

    #[test]
    fn the_cli_config_file_is_used_as_given() {
        let dir = Some(PathBuf::from("/home/u/.config"));
        assert_eq!(
            config_file_in(Some(PathBuf::from("my.toml")), dir).unwrap(),
            PathBuf::from("my.toml")
        );
        assert_eq!(
            config_file_in(Some(PathBuf::from("/a/b.toml")), None).unwrap(),
            PathBuf::from("/a/b.toml")
        );
    }

    #[test]
    fn the_default_config_file_is_in_the_config_directory() {
        let dir = Some(PathBuf::from("/home/u/.config"));
        assert_eq!(
            config_file_in(None, dir).unwrap(),
            PathBuf::from("/home/u/.config/fbarcalc/config.toml")
        );
    }

    #[test]
    fn no_config_directory_is_an_error_unless_a_file_was_given() {
        assert!(matches!(
            config_file_in(None, None),
            Err(ConfigError::NoConfigDir)
        ));
        assert!(config_file_in(Some(PathBuf::from("x.toml")), None).is_ok());
    }

    // parsing

    #[test]
    fn parse_a_complete_config() {
        let c = parse_config(
            "version = 1\n\
             default_input_currency = \"GBP\"\n\
             fca_api_key = \"abc123\"\n",
        )
        .unwrap();

        assert_eq!(c.version, 1);
        assert_eq!(c.default_input_currency.as_deref(), Some("GBP"));
        assert_eq!(c.fca_api_key.as_deref(), Some("abc123"));
    }

    #[test]
    fn parse_an_empty_config() {
        let c = parse_config("").unwrap();

        assert_eq!(c.version, CONFIG_VERSION);
        assert_eq!(c.default_input_currency, None);
        assert_eq!(c.fca_api_key, None);
    }

    #[test]
    fn parse_a_partial_config() {
        let c = parse_config("fca_api_key = \"abc123\"\n").unwrap();

        assert_eq!(c.version, CONFIG_VERSION);
        assert_eq!(c.default_input_currency, None);
        assert_eq!(c.fca_api_key.as_deref(), Some("abc123"));
    }

    #[test]
    fn parse_ignores_unknown_settings_and_comments() {
        let c = parse_config(
            "# my config\nfuture = true\ndefault_input_currency = \"JPY\"\n",
        )
        .unwrap();

        assert_eq!(c.default_input_currency.as_deref(), Some("JPY"));
    }

    #[test]
    fn parse_keeps_an_unsupported_currency_for_the_prompt_to_deal_with() {
        let c = parse_config("default_input_currency = \"CAD\"\n").unwrap();
        assert_eq!(c.default_input_currency.as_deref(), Some("CAD"));
    }

    #[test]
    fn parse_rejects_settings_with_the_wrong_type() {
        for (toml, wanted_key) in [
            ("version = \"1\"", "version"),
            ("version = 1.5", "version"),
            ("default_input_currency = 5", CONFIG_DEFAULT_INPUT_CURRENCY),
            (
                "default_input_currency = [\"EUR\"]",
                CONFIG_DEFAULT_INPUT_CURRENCY,
            ),
            ("fca_api_key = true", CONFIG_FCA_API_KEY),
            ("fca_api_key = 12345", CONFIG_FCA_API_KEY),
        ] {
            match parse_config(toml) {
                Err(ConfigError::WrongType { key, .. }) => {
                    assert_eq!(key, wanted_key, "{}", toml)
                }
                other => panic!("{:?}: {:?}", toml, other),
            }
        }
    }

    #[test]
    fn parse_rejects_invalid_toml() {
        for toml in ["not toml", "version = ", "[unclosed", "a = 1\na = 2"] {
            assert!(
                matches!(parse_config(toml), Err(ConfigError::Parse(_))),
                "{:?}",
                toml
            );
        }
    }

    #[test]
    fn wrong_type_messages_name_the_setting() {
        let e = parse_config("fca_api_key = 5").unwrap_err();
        assert_eq!(e.to_string(), "`fca_api_key` must be a string");
    }

    // reading the config file

    #[test]
    fn get_config_reads_the_file() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");
        std::fs::write(
            &file,
            "default_input_currency = \"EUR\"\nfca_api_key = \"k\"\n",
        )
        .unwrap();

        let c = get_config(Some(file)).unwrap();
        assert_eq!(c.default_input_currency.as_deref(), Some("EUR"));
        assert_eq!(c.fca_api_key.as_deref(), Some("k"));
    }

    #[test]
    fn get_config_says_how_to_fix_a_missing_file() {
        let dir = TempDir::new();
        let file = dir.join("missing.toml");

        let e = get_config(Some(file.clone())).unwrap_err();
        assert!(matches!(
            e,
            ConfigError::File {
                ref source, ..
            } if matches!(**source, ConfigError::NotFound)
        ));

        let msg = e.to_string();
        assert!(msg.starts_with(&file.display().to_string()), "{}", msg);
        assert!(msg.contains("fbarcalc config"), "{}", msg);
    }

    #[test]
    fn get_config_reports_the_file_with_a_problem() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");

        std::fs::write(&file, "fca_api_key = 5\n").unwrap();
        let msg = get_config(Some(file.clone())).unwrap_err().to_string();
        assert!(msg.starts_with(&file.display().to_string()), "{}", msg);
        assert!(msg.contains("fca_api_key"), "{}", msg);

        std::fs::write(&file, "this is not toml\n").unwrap();
        let msg = get_config(Some(file.clone())).unwrap_err().to_string();
        assert!(msg.starts_with(&file.display().to_string()), "{}", msg);
    }

    #[test]
    fn get_config_fails_without_panicking_if_the_path_is_a_directory() {
        let dir = TempDir::new();
        assert!(get_config(Some(dir.path().to_path_buf())).is_err());
    }

    #[test]
    fn read_if_exists_distinguishes_missing_from_empty() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");

        assert_eq!(read_if_exists(&file).unwrap(), None);
        std::fs::write(&file, "").unwrap();
        assert_eq!(read_if_exists(&file).unwrap().as_deref(), Some(""));
    }

    // updating the config

    #[test]
    fn update_creates_a_config_from_nothing() {
        let toml = update_config_doc("", "EUR", Some("abc123")).unwrap();
        let c = parse_config(&toml).unwrap();

        assert_eq!(c.version, CONFIG_VERSION);
        assert_eq!(c.default_input_currency.as_deref(), Some("EUR"));
        assert_eq!(c.fca_api_key.as_deref(), Some("abc123"));
    }

    #[test]
    fn update_keeps_the_existing_key_if_no_new_one_is_given() {
        let existing =
            "default_input_currency = \"EUR\"\nfca_api_key = \"old\"\n";

        for key in [None, Some("")] {
            let toml = update_config_doc(existing, "GBP", key).unwrap();
            let c = parse_config(&toml).unwrap();

            assert_eq!(c.default_input_currency.as_deref(), Some("GBP"));
            assert_eq!(c.fca_api_key.as_deref(), Some("old"));
        }
    }

    #[test]
    fn update_replaces_the_key_if_a_new_one_is_given() {
        let existing = "fca_api_key = \"old\"\n";
        let toml = update_config_doc(existing, "USD", Some("new")).unwrap();

        assert_eq!(
            parse_config(&toml).unwrap().fca_api_key.as_deref(),
            Some("new")
        );
        assert!(!toml.contains("old"));
    }

    #[test]
    fn update_without_a_key_does_not_invent_one() {
        let toml = update_config_doc("", "USD", None).unwrap();
        assert_eq!(parse_config(&toml).unwrap().fca_api_key, None);
        assert!(!toml.contains(CONFIG_FCA_API_KEY));
    }

    #[test]
    fn update_upgrades_the_version() {
        let toml = update_config_doc("version = 0\n", "USD", None).unwrap();
        assert_eq!(parse_config(&toml).unwrap().version, CONFIG_VERSION);
    }

    #[test]
    fn update_preserves_comments_and_other_settings() {
        let existing = "\
# my notes about this file
future_setting = \"keep me\"

# the currency I use most
default_input_currency = \"EUR\"
";
        let toml = update_config_doc(existing, "JPY", Some("k")).unwrap();

        assert!(toml.contains("# my notes about this file"), "{}", toml);
        assert!(toml.contains("# the currency I use most"), "{}", toml);
        assert!(toml.contains("future_setting = \"keep me\""), "{}", toml);
        assert_eq!(
            parse_config(&toml)
                .unwrap()
                .default_input_currency
                .as_deref(),
            Some("JPY")
        );
    }

    #[test]
    fn update_refuses_to_overwrite_a_config_that_is_not_toml() {
        assert!(matches!(
            update_config_doc("this is not toml", "USD", Some("k")),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn update_is_idempotent() {
        let once = update_config_doc("", "EUR", Some("k")).unwrap();
        let twice = update_config_doc(&once, "EUR", Some("k")).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn the_key_is_masked_with_one_star_per_character() {
        assert_eq!(mask_key("abc123", None), "******");
        assert_eq!(mask_key("abc123", Some("old")), "******");
        // characters, not bytes
        assert_eq!(mask_key("pässwörd", None), "********");
    }

    #[test]
    fn an_empty_key_is_masked_like_the_key_it_keeps() {
        assert_eq!(mask_key("", Some("old-key")), "*******");
        assert_eq!(mask_key("", None), "");
    }

    // writing the config file

    #[test]
    fn write_creates_the_file_and_its_directories() {
        let dir = TempDir::new();
        let file = dir.join("a/b/fbarcalc/config.toml");

        write_config(&file, "version = 1\n").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "version = 1\n");
    }

    #[test]
    fn write_creates_a_file_only_we_can_read() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");

        write_config(&file, "fca_api_key = \"secret\"\n").unwrap();

        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn write_replaces_the_whole_file() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");

        write_config(&file, "a long long long long first version\n").unwrap();
        write_config(&file, "short\n").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "short\n");
    }

    #[test]
    fn write_reports_an_error_instead_of_panicking() {
        let dir = TempDir::new();
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "i am a file").unwrap();

        // the parent "directory" is a file
        let e = write_config(&blocker.join("config.toml"), "x").unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("blocker"), "{}", msg);

        // the file itself is a directory
        assert!(write_config(dir.path(), "x").is_err());
    }

    #[test]
    fn a_file_in_the_current_directory_needs_no_directory_created() {
        assert!(create_config_directory(Path::new("config.toml")).is_ok());
        assert!(create_config_directory(Path::new("./config.toml")).is_ok());
        assert!(create_config_directory(Path::new("/")).is_ok());
    }

    // error reporting

    #[test]
    fn a_missing_config_directory_suggests_choosing_a_file() {
        let msg = ConfigError::NoConfigDir.to_string();
        assert!(msg.contains("--config"), "{}", msg);
    }

    #[test]
    fn errors_know_their_underlying_cause() {
        let io = ConfigError::Io(std::io::Error::other("disk on fire"));
        assert_eq!(io.source().unwrap().to_string(), "disk on fire");

        let parse = parse_config("not toml").unwrap_err();
        assert!(matches!(parse, ConfigError::Parse(_)));
        assert!(parse.source().is_some());

        let file = ConfigError::File {
            path: PathBuf::from("c.toml"),
            source: Box::new(ConfigError::NotFound),
        };
        assert_eq!(
            file.to_string(),
            format!("c.toml: {}", ConfigError::NotFound)
        );
        assert_eq!(
            file.source().unwrap().to_string(),
            ConfigError::NotFound.to_string()
        );

        for e in [
            ConfigError::NoConfigDir,
            ConfigError::NotFound,
            ConfigError::WrongType {
                key: "version",
                expected: "an integer",
            },
        ] {
            assert!(e.source().is_none(), "{:?}", e);
        }
    }

    // saving the answers

    #[test]
    fn saving_creates_a_new_config() {
        let dir = TempDir::new();
        let file = dir.join("fbarcalc/config.toml");

        save_config(&file, "", "GBP", "abc123").unwrap();

        let c = get_config(Some(file)).unwrap();
        assert_eq!(c.version, CONFIG_VERSION);
        assert_eq!(c.default_input_currency.as_deref(), Some("GBP"));
        assert_eq!(c.fca_api_key.as_deref(), Some("abc123"));
    }

    #[test]
    fn saving_updates_an_existing_config_and_keeps_the_rest() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");
        let existing = "# mine\nfuture = 1\ndefault_input_currency = \"EUR\"\n\
                        fca_api_key = \"old\"\n";
        std::fs::write(&file, existing).unwrap();

        // an empty answer for the key keeps the one that we have
        save_config(&file, existing, "JPY", "").unwrap();

        let saved = std::fs::read_to_string(&file).unwrap();
        assert!(saved.contains("# mine"), "{}", saved);
        assert!(saved.contains("future = 1"), "{}", saved);

        let c = parse_config(&saved).unwrap();
        assert_eq!(c.default_input_currency.as_deref(), Some("JPY"));
        assert_eq!(c.fca_api_key.as_deref(), Some("old"));
    }

    #[test]
    fn saving_reports_which_file_could_not_be_written() {
        let dir = TempDir::new();

        // the config "file" is a directory
        let msg = save_config(dir.path(), "", "EUR", "k")
            .unwrap_err()
            .to_string();
        assert!(
            msg.starts_with(&dir.path().display().to_string()),
            "{}",
            msg
        );
    }

    #[test]
    fn saving_refuses_to_replace_a_config_that_is_not_toml() {
        let dir = TempDir::new();
        let file = dir.join("config.toml");
        std::fs::write(&file, "not toml").unwrap();

        assert!(save_config(&file, "not toml", "EUR", "k").is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "not toml");
    }
}
