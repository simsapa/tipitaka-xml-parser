# Tipitaka-xml Parser

A cli tool which attempts to parse [tipitaka-xml](https://github.com/VipassanaTech/tipitaka-xml) `romn/` xml files into suttas.

The [Simsapa Dhamma Reader](https://github.com/simsapa/simsapa-ng) uses it to
bootstrap the CST4 suttas into its database.

The cli tool produces an sqlite database. The `xml_fragments` table contains
suttas with their metadata, the xml slice and its start- and end position in the
original xml file.

Thus the original xml file can be reconstructed from the `xml_fragments` rows.

The parsed metadata includes the `cst_code (sn5.12.2.1)` (CST4 numbering) and
the corresponding `sc_code (sn56.11)`, which is the Wisdom Publications
numbering adopted by [SuttaCentral](https://suttacentral.net/).

