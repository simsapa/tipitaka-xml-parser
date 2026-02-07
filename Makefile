db-remove-and-restore:
	rm ./data/ -r; cp -r ../tipitaka-xml-data/data-2026-02-03/ ./data

db-remove-and-restore-only-db:
	rm ./data/*; cp ../tipitaka-xml-data/data-2026-02-03/fragments.sqlite3 ./data

tsv-before:
	cargo run -- export-fragments-to-tsv ./data/fragments.sqlite3 before.tsv

tsv-after:
	cargo run -- export-fragments-to-tsv ./data/fragments.sqlite3 after.tsv
