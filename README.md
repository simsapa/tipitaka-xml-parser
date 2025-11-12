# Tipitaka-xml Parser

A cli tool which attempts to parse [tipitaka-xml](https://github.com/VipassanaTech/tipitaka-xml) `romn/` xml files into suttas in an sqlite database.

⚠️ **Warning** ⚠️ **Work-In-Progress.** The parser produces some okay results for
some nikāyas (tested with the xml in [tests/data/](tests/data/)), but it still
can't handle many cases due to the irregular structure of the xml files.

There is still room for improving the parser results with with more code to
handle various cases.

After that, manual checking and correction will be necessary, I am thinking a
small web GUI could help to quickly move through the rows, typing in missing
values and adjusting xml fragment boundaries.

## Use-case

The intended result would be an sqlite database which sutta reader apps can use
to import CST4 suttas (mūla, aṭṭhakathā, ṭīkā) with SuttaCentral reference codes
where possible.

[Simsapa Dhamma Reader](https://github.com/simsapa/simsapa-ng) uses it to bootstrap the CST4 suttas into its database.

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
cargo run -- parse-tipitaka-xml --xml-file path/to/tipitaka-xml/romn/s0201m.mul.xml --fragments-db fragments.sqlite3
```

## Web UI for Fragment Review

The project includes a web-based user interface for reviewing and correcting fragment boundaries and metadata.

### Starting the Web UI

```bash
cargo run -- web-ui path/to/fragments.sqlite3
```

By default, the server runs on port 8000. You can specify a different port:

```bash
cargo run -- web-ui path/to/fragments.sqlite3 --port 8080
```

Then open your browser to `http://localhost:8000` (or the specified port).

### Settings and Configuration

The web UI includes a Settings menu (accessible from the top-right menu button) where you can configure:

- **Fragments Database Path**: Path to your SQLite database file
- **Server Port**: Port number for the web server (requires restart to take effect)
- **XML Files Paths**: List of XML files being processed (for reference)

Settings are persisted in a `web-ui-config.toml` file in the current directory. An example config file is provided as `web-ui-config.toml.example`.

### UI State Persistence

The web UI automatically saves your current position (selected file and fragment) in browser localStorage, so when you reload the page, it will restore your previous state.

### Features

- Browse XML files and their parsed fragments
- View fragment content with previous/next context
- Edit fragment metadata (CST codes, review status, etc.)
- Adjust fragment boundaries by moving lines or characters
- Delete and merge fragments
- Mark fragments with review status (unchecked, in-progress, checked, needs-review)

