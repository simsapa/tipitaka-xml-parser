# Tipitaka-xml Parser

A cli tool which attempts to parse [tipitaka-xml](https://github.com/VipassanaTech/tipitaka-xml) `romn/` xml files into suttas in an sqlite database.

⚠️ **Warning** ⚠️ **Work-In-Progress.** The parser produces some okay results for some nikāyas, while it can't handle some others due to the irregular structure of the xml files.

The parser results can be still improved with extra code to handle various cases, after which manual checking and correction will be necessary.

[Simsapa Dhamma Reader](https://github.com/simsapa/simsapa-ng) uses it to
bootstrap the CST4 suttas into its database.

The cli tool produces an sqlite database. The `xml_fragments` table contains
suttas with their metadata, the xml slice and its start- and end position in the
original xml file.

Thus the original xml files can be reconstructed from the `xml_fragments` rows.

The parsed metadata includes the `cst_code (sn5.12.2.1)` (CST4 numbering) and
the corresponding `sc_code (sn56.11)`, which is the Wisdom Publications
numbering adopted by [SuttaCentral](https://suttacentral.net/).

![xml fragments db](docs/xml-fragments-db-screenshot.png)

## Example

It can be called directly on the `romn/*.xml` files of [tipitaka-xml](https://github.com/VipassanaTech/tipitaka-xml).
Note that these are in UTF-16 encoding.

There are test xml files in UTF-8 encoding in the `tests/data/` folder of this repo.

```
cargo run -- parse-tipitaka-xml path/to/tipitaka-xml/romn/s0201m.mul.xml --fragments-db fragments.sqlite3
```

