# OkLink tax script for UniSat chain

This is a script for fetching inscriptions from the [UniSat] chain using the
[OkLink API], processing them, then writing the results to a CSV according to
[CryptoTaxCalculator][ctc]'s CSV formatting.

This script is very limited in scope, it can only handle the transaction types
that the author has used. However, it should be easy to extend to handle other
transaction types as well as other output formats fairly easily.

## Usage

To use the script, simply run the application, passing the [oklink api] key and
your BTC wallet as arguments, e.g.:

```console
$ cargo run -- <OKLINK_API_KEY> <BTC_WALLET>
```

[ctc]: https://help.cryptotaxcalculator.io/en/articles/5777675-advanced-manual-custom-csv-import
[oklink api]: https://www.oklink.com/docs/en/#explorer-api-brc20
[unisat]: https://unisat.io/
