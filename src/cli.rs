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

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE", global = true)]
    pub config: Option<PathBuf>,

    /// Show the balance after each transaction and the exchange rate used
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Interactive configuration
    ///
    /// Runs an interactive configuration prompt to set common values and
    /// e.g, API keys for doing currency conversion.
    Config {},
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(
            std::iter::once("fbarcalc").chain(args.iter().copied()),
        )
    }

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn no_arguments_runs_the_calculation() {
        let cli = parse(&[]).unwrap();

        assert!(cli.command.is_none());
        assert_eq!(cli.config, None);
        assert!(!cli.verbose);
    }

    #[test]
    fn the_config_subcommand() {
        let cli = parse(&["config"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Config {})));
    }

    #[test]
    fn the_config_file_can_be_given_in_any_of_the_usual_ways() {
        for args in [
            &["-c", "x.toml"][..],
            &["--config", "x.toml"],
            &["--config=x.toml"],
            &["-cx.toml"],
        ] {
            let cli = parse(args).unwrap();
            assert_eq!(cli.config, Some(PathBuf::from("x.toml")), "{:?}", args);
            assert!(cli.command.is_none());
        }
    }

    #[test]
    fn the_config_file_can_come_before_or_after_the_subcommand() {
        for args in [
            &["-c", "x.toml", "config"][..],
            &["config", "-c", "x.toml"],
            &["--config", "x.toml", "config"],
            &["config", "--config", "x.toml"],
        ] {
            let cli = parse(args).unwrap();
            assert_eq!(cli.config, Some(PathBuf::from("x.toml")), "{:?}", args);
            assert!(
                matches!(cli.command, Some(Commands::Config {})),
                "{:?}",
                args
            );
        }
    }

    #[test]
    fn a_config_file_needs_a_value() {
        assert!(parse(&["-c"]).is_err());
        assert!(parse(&["--config"]).is_err());
    }

    #[test]
    fn verbose_can_be_short_or_long() {
        assert!(parse(&["-v"]).unwrap().verbose);
        assert!(parse(&["--verbose"]).unwrap().verbose);
    }

    #[test]
    fn verbose_goes_with_a_config_file() {
        let cli = parse(&["-v", "-c", "x.toml"]).unwrap();

        assert!(cli.verbose);
        assert_eq!(cli.config, Some(PathBuf::from("x.toml")));
    }

    #[test]
    fn verbose_is_for_the_calculation_and_not_for_config() {
        assert!(parse(&["config", "-v"]).is_err());
        assert!(parse(&["config", "--verbose"]).is_err());
    }

    #[test]
    fn version_and_help_are_not_confused_with_verbose() {
        let version = parse(&["-V"]).err().unwrap();
        assert_eq!(version.kind(), ErrorKind::DisplayVersion);

        let help = parse(&["--help"]).err().unwrap();
        assert_eq!(help.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(parse(&["--debug"]).is_err());
        assert!(parse(&["nonsense"]).is_err());
    }
}
