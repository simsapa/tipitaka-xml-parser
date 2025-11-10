#!/usr/bin/env bash

set -e

# This checks that the script is executed from the correct folder.
if [ ! -f xml_list.txt ]; then
    echo "This script must be executed from the same folder as xml_list.txt"
    exit 2
fi

if [ -f fragments.sqlite3 ]; then
    rm fragments.sqlite3
fi

CUR_DIR="$PWD"

XML_LIST_PATH=xml_list.txt
FRAGMENTS_DB_PATH=fragments.sqlite3
FRAGMENTS_TSV_PATH=fragments.tsv

# NOTE: No trailing slash
XML_PARSER_DIR=..
TIPITAKA_XML_ROMN_DIR=/home/gambhiro/prods/apps/simsapa-ng-project/bootstrap-assets-resources/tipitaka-org-vri-cst/tipitaka-xml/romn

cat "$XML_LIST_PATH" | grep -vE '^#|^\s*$' | sed "s|^|${TIPITAKA_XML_ROMN_DIR}/|" > $XML_LIST_PATH.full

cd "$XML_PARSER_DIR"
cargo build

cd "$CUR_DIR"
ENABLE_PRINT_LOG=false $XML_PARSER_DIR/target/debug/tipitaka_xml_parser parse-tipitaka-xml --xml-list "$XML_LIST_PATH".full --fragments-db "$FRAGMENTS_DB_PATH"

ENABLE_PRINT_LOG=false $XML_PARSER_DIR/target/debug/tipitaka_xml_parser export-fragments-to-tsv "$FRAGMENTS_DB_PATH" "$FRAGMENTS_TSV_PATH"

diff -q "$FRAGMENTS_TSV_PATH" "$FRAGMENTS_TSV_PATH".reference
