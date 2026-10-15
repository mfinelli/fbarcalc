# fbarcalc

This is a quick program that I wrote to help me fill out my annual Foreign
Bank and Financial Accounts (FBAR) report.

Given a starting balance and a list of transactions you can input them and at
the end receive the maximum account balance as well as a conversion to (the
current exchange rate) USD.

- https://www.irs.gov/businesses/small-businesses-self-employed/report-of-foreign-bank-and-financial-accounts-fbar
- https://bsaefiling.fincen.treas.gov/main.html

> [!IMPORTANT]
>
> **N.B.** While you may find this program useful I wrote it for my own
> personal use. I am not an accountant and am not liable for any outcome
> resulting from your usage of the program. You should be using your own,
> licensed CPA. See the LICENSE for details.

## usage

You can build it if you have the rust toolchain installed:

```shell
cargo build --release
```

Then you can run the configuration where you will select the default input
currency (which will be confirmed on each run) and an API key from
[freecurrencyapi.com](https://freecurrencyapi.com) and is used to convert the
input currency to USD at the end of the process. At the time of writing the
free tier is 5000 requests per month which should be more than enough for the
personal usage once per year. The API key isn't needed if the input currency is
USD.

```shell
fbarcalc config
```

Once configured you can launch the program to run a calculation.

```shell
fbarcalc
```

![Demo](fbarcalc.gif)

To see how the maximum was reached use `--verbose` (or `-v`). After you have
entered the transactions you also get the balance after each of them, with the
maximum marked, and the exchange rate that was used for the conversion, so
that you can check the numbers:

```
#  Transaction    Balance
0        start  €1,500.00
1      €320.75  €1,820.75
2   -€1,200.00    €620.75
3       €80.10    €700.85
4    €2,000.00  €2,700.85  <- maximum
5     -€450.00  €2,250.85

Final balance in EUR: €2,250.85
Maximum value in EUR: €2,700.85
Exchange rate: 1 EUR = 1.0842 USD
Maximum value in USD: $2,928.26
```

### docker

There's also a docker image available, you either need to pass the
configuration as a volume or launch the image with `bash` and then run the
configure and run steps separately:

```shell
docker run --rm -it mfinelli/fbarcalc bash
```

## development

To regenerate the [asciinema](https://asciinema.org):

```shell
asciinema rec fbarcalc.cast -c \
  "bash -c 'fbarcalc config && echo && fbarcalc'" -t fbarcalc
agg fbarcalc.cast fbarcalc.gif
```

## license

Licensed under the GNU GPL version 3.0 or later.

```
fbarcalc: find account maxium value
Copyright 2024-2026 Mario Finelli

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```
