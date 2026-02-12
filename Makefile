web-ui:
	cargo run -- web-ui

db-remove-and-restore:
	rm ./data/ -r; cp -r ../tipitaka-xml-data/data-2026-02-03/ ./data

regenerate:
	cargo run -- regenerate

# Copy from the older db because it has "checked" and empty rows to test that regeneration works the same.
db-remove-and-restore-only-db:
	rm ./data/*; cp ../tipitaka-xml-data/data-2026-02-03/fragments.sqlite3 ./data

# Diff with the results of the previously known db's tsv after its regeneration (before a new one), with the tsv result after the current regeneration.
make diff-after:
	diff ../tipitaka-xml-data/data-2026-02-08/fragments.before.tsv after.tsv

tsv-before:
	cargo run -- export-fragments-to-tsv ./data/fragments.sqlite3 before.tsv

tsv-after:
	cargo run -- export-fragments-to-tsv ./data/fragments.sqlite3 after.tsv
