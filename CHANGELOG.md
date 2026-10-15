# fbarcalc changelog

This document keeps track of major changes to the `fbarcalc` program.

## Unreleased

- Show the final balance after the last transaction, so that it can be
  compared with the statement that the transactions came from.
- Show amounts with thousands separators (`1,234.56`) in the prompts as well
  as in the results.

## 2026-10-15: v1.0.2

- Fail with an error if the input isn't a terminal. Before, a program started
  with its input redirected would wait for someone to type at the terminal
  that it was started from if there was one, and never finish.

## 2026-10-15: v1.0.1

Upgrade dependencies to resolve a packaging problem on Arch Linux.

## 2026-10-15: v1.0.0

First stable release. Now published to [crates.io](https://crates.io) using
[trusted publishing](https://crates.io/docs/trusted-publishing).

## 2026-10-15: v0.3.0

- Fix amounts being read one cent too low: floating point error turned values
  such as `8.2` and `19.99` into `8.19` and `19.98`. Amounts are now exact and
  anything after the second decimal place is truncated towards zero.
- Only plain decimal amounts are accepted: `nan`, `inf` and exponents like
  `1e3` are rejected (`nan` used to crash the program).
- Add `--verbose` (`-v`, which used to be accepted but did nothing). After the
  transactions are entered it also shows the balance after each of them, with
  the maximum marked, and the exchange rate that was used.
- Report problems with an error message and exit status 1 instead of crashing:
  a missing, unreadable or invalid config file, a setting of the wrong type, a
  default currency that isn't supported, Ctrl-C or Esc at a prompt, and
  failures of the exchange rate service. `fbarcalc config` also refuses to
  overwrite a config file that isn't valid TOML.
- Don't show the API key in the error output when the exchange rate request
  fails: it used to be printed as part of the request URL.
- USD doesn't need an API key or an exchange rate request anymore. For other
  currencies a missing API key is reported before asking for the transactions,
  and if the conversion fails the maximum in the input currency is still shown.
- `fbarcalc config` now works when the config directory doesn't exist yet, and
  `-c`/`--config` can be given after it (`fbarcalc config -c FILE`).
- Show negative amounts as `-$5.00` instead of `$-5.00` and round the
  conversion to USD to the nearest cent.
- Use rustls instead of OpenSSL for HTTPS (`reqwest` 0.13).

## 2025-07-22: v0.2.0

Add macos release

## 2024-09-27: v0.1.0

Initial release
