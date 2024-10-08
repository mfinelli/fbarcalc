/* fbarcalc: find account maximum value
 * Copyright 2024 Mario Finelli
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

use inquire::{Password, Select};
use std::error::Error;
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

#[derive(Clone, Debug)]
pub struct Config {
    pub version: i64,
    pub default_input_currency: Option<String>,
    pub fca_api_key: Option<String>,
}

pub fn get_config_file(cli_config: Option<PathBuf>) -> PathBuf {
    match cli_config {
        Some(c) => c,
        None => {
            let mut config_path = dirs::config_dir().unwrap();
            config_path.push("fbarcalc");
            config_path.push("config.toml");
            config_path
        }
    }
}

pub fn get_config(config: Option<PathBuf>) -> Config {
    let config_file = get_config_file(config);
    match config_file.try_exists() {
        Ok(e) => {
            if e {
                let toml = std::fs::read_to_string(&config_file).unwrap();
                let doc = toml.parse::<DocumentMut>().unwrap();

                let v = match doc.get("version") {
                    None => CONFIG_VERSION,
                    Some(i) => i.as_integer().unwrap(),
                };

                let c = doc
                    .get(CONFIG_DEFAULT_INPUT_CURRENCY)
                    .map(|s| s.as_str().unwrap().to_string());

                let k = doc
                    .get(CONFIG_FCA_API_KEY)
                    .map(|s| s.as_str().unwrap().to_string());

                Config {
                    version: v,
                    default_input_currency: c,
                    fca_api_key: k,
                }
            } else {
                panic!("config file doesn't exist yet..."); // TODO: do better
            }
        }
        Err(_) => panic!("there was an error checking the config file"), // TODO: do better
    }
}

fn create_config_directory(config_file: &Path) -> Result<(), std::io::Error> {
    let p = config_file.parent().unwrap();
    if p == Path::new("") || p == Path::new(".") || p == Path::new("..") {
        return Ok(());
    }

    match p.try_exists() {
        Err(e) => Err(e),
        Ok(e) => {
            if e {
                Ok(())
            } else {
                std::fs::create_dir(p)
            }
        }
    }
}

pub fn select_input_currency(
    default: Option<&str>,
    is_default: bool,
) -> Result<&Currency, Box<dyn Error>> {
    let options = SUPPORTED_CURRENCIES
        .iter()
        .map(|c| c.name)
        .collect::<Vec<_>>();
    let start = match default {
        // TODO: handle one that we don't have configured
        Some(code) => SUPPORTED_CURRENCIES
            .iter()
            .position(|c| c.code == code)
            .unwrap(),
        None => 0,
    };

    let p = if is_default {
        "What is the default input currency?"
    } else {
        "What is the input currency?"
    };

    let ans = Select::new(p, options).with_starting_cursor(start).prompt();
    match ans {
        Ok(choice) => Ok(SUPPORTED_CURRENCIES
            .iter()
            .find(|c| c.name == choice)
            .unwrap()),
        Err(e) => panic!("{}", e), // TODO: do better
    }
}

pub fn configure(config: Option<PathBuf>) -> ExitCode {
    let config_file = get_config_file(config);
    let config_exists = match config_file.try_exists() {
        Ok(e) => e,
        Err(_) => {
            println!("error: there was an error checking the config file");
            return ExitCode::FAILURE;
        }
    };

    let toml = if config_exists {
        std::fs::read_to_string(&config_file).unwrap()
    } else {
        "".to_string()
    };

    let mut new_doc = toml.parse::<DocumentMut>().unwrap();
    let doc = toml.parse::<DocumentMut>().unwrap();

    let c = match doc.get(CONFIG_DEFAULT_INPUT_CURRENCY) {
        None => select_input_currency(None, true).unwrap(),
        Some(s) => select_input_currency(s.as_str(), true).unwrap(),
    };

    let key = Password::new("Freecurrency.com API key:")
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_formatter(&|k| {
            if k.is_empty() {
                match doc.get(CONFIG_FCA_API_KEY) {
                    None => String::from(""),
                    Some(s) => "*".repeat(s.as_str().unwrap().chars().count()),
                }
            } else {
                "*".repeat(k.chars().count())
            }
        })
        .without_confirmation()
        .prompt();

    new_doc["version"] = toml_edit::value(CONFIG_VERSION);
    new_doc[CONFIG_DEFAULT_INPUT_CURRENCY] = toml_edit::value(c.code);

    match key {
        Ok(v) => {
            if !v.is_empty() {
                new_doc[CONFIG_FCA_API_KEY] = toml_edit::value(v);
            }
        }
        Err(_) => {
            println!("error: there was an error getting the API key");
            return ExitCode::FAILURE;
        }
    };

    // TODO: don't unwrap these but handle the error and retun ExitCode::FAILURE
    create_config_directory(&config_file).unwrap();
    let mut file: std::fs::File = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(config_file)
        .unwrap();
    file.write_all(new_doc.to_string().as_bytes()).unwrap();

    ExitCode::SUCCESS
}
