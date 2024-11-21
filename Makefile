dev: static
	find src/ static/ | entr -sr 'cargo sqlx prepare --database-url sqlite:accounts.db && cargo run --features hotreload serve --database accounts.db'

static: static/htmx.js static/pico.css static/pico.colors.css

.PHONY: dev static

static/htmx.js:
	curl -Lf https://unpkg.com/htmx.org@2.0.3 -o $@

static/pico.css:
	curl -Lf https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.violet.min.css -o $@

static/pico.colors.css:
	curl -Lf https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.colors.min.css -o $@
