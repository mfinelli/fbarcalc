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

use inquire::Select;
use std::error::Error;
use std::path::PathBuf;
use std::path::Path;
use toml_edit::DocumentMut;
use std::process::ExitCode;

const CONFIG_VERSION: i64 = 1;

const CONFIG_DEFAULT_INPUT_CURRENCY: &str = "default_input_currency";

#[derive(Clone, Debug)]
pub struct Currency<'a> {
    code: &'a str,
    name: &'a str,
    symbol: &'a str,
}

#[derive(Clone, Debug)]
pub struct Config<'a> {
    pub version: u8,
    pub default_input_currency: Option<&'a str>,
}

/// Returns a vector of the currencies that we're currently supporting.
pub fn supported_currencies() -> Vec<Currency<'static>> {
    vec![
        Currency {
            code: "EUR",
            name: "Euro",
            symbol: "€",
        },
        Currency {
            code: "GBP",
            name: "British Pound Sterling",
            symbol: "£",
        },
        Currency {
            code: "JPY",
            name: "Japanese Yen",
            symbol: "¥",
        },
        Currency {
            code: "USD",
            name: "US Dollar",
            symbol: "$",
        },
    ]
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

fn create_config_directory(config_file: PathBuf) -> Result<(), std::io::Error> {
    let p = config_file.parent().unwrap();
    if p == Path::new("") || p == Path::new(".") || p == Path::new("..") {
        return Ok(());
    }

    match p.try_exists() {
        Err(e) => Err(e),
        Ok(e) => if e {
            Ok(())
        } else {
            std::fs::create_dir(p)
        }
    }
}

/// A function that we can use in other routines to make sure that we've
/// written a config file first. Different from what we use in the config
/// module because here we don't want to error if it doesn't exist... we're
/// going to write it out now!
// pub fn check_if_config_exists(config_file: PathBuf) -> Result<bool, Box<dyn Error>> {
//     let result = config_file.try_exists();
//     if result.is_err() {
//         return Err(

//     Ok(false)
// }

pub fn select_input_currency(default: Option<&str>) -> Result<Currency, Box<dyn Error>> {
    let options = supported_currencies().iter().map(|c| c.name).collect::<Vec<_>>();
    let start = match default {
        // TODO: handle one that we don't have configured
        Some(code) => supported_currencies().iter().position(|c| c.code == code).unwrap(),
        None => 0,
    };

    let ans = Select::new("What is the default input currency?",options).with_starting_cursor(start).prompt();
    match ans {
        Ok(choice) => Ok(supported_currencies().iter().find(|c| c.name == choice).unwrap().clone()),
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

    // let mut doc = if config_exists {
    //     let toml = std::fs::read_to_string(&config_file).unwrap();
    //     toml.parse::<DocumentMut>().unwrap()
    // } else {
    //     "".parse::<DocumentMut>().unwrap()
    // };
    // let c: Currency;

    let c = match doc.get(CONFIG_DEFAULT_INPUT_CURRENCY) {
        None => select_input_currency(None).unwrap(),
        Some(s) => select_input_currency(s.as_str()).unwrap(),
    };

    // println!("{:?}", c);

    new_doc["version"] = toml_edit::value(CONFIG_VERSION);
    new_doc[CONFIG_DEFAULT_INPUT_CURRENCY] = toml_edit::value(c.code);

    // if config_exists {
    //     let toml = std::fs::read_to_string(&config_file).unwrap();
    //     doc = toml.parse::<DocumentMut>().unwrap();
    //     let copy = toml.parse::<DocumentMut>().unwrap();

    //     let def = match copy.get(CONFIG_DEFAULT_INPUT_CURRENCY) {
    //         None => None,
    //         Some(s) => s.as_str(),
    //     };
    //     // let d = doc.get(CONFIG_DEFAULT_INPUT_CURRENCY).c;

    //     c = select_input_currency(def).unwrap();
    // } else {
    //     c = select_input_currency(None).unwrap();
    // }

    // we've already checked that we can access the path so just unwrap it
    // if config_file.try_exists().unwrap() {
    //     // do the parsing....

    //     c = select_input_currency(None).unwrap();
    // } else {
        // c = select_input_currency(None).unwrap();
    // }

    // let new_config = Config {
    //     version: CONFIG_VERSION,
    //     default_input_currency: Some(c.code),
    // };
    //
    // doc["version"] = toml_edit::value(CONFIG_VERSION);
    // doc["default_input_currency"] = toml_edit::value(c.code.clone());
    println!("{}", new_doc.to_string());

    let _ = create_config_directory(config_file);

    return ExitCode::SUCCESS;
}
