.PHONY: check check-core check-web check-container run web fmt sample-form

check:
	mise exec -- cargo fmt --all -- --check
	mise exec -- cargo clippy --all-features --all-targets -- -D warnings
	mise exec -- cargo test --all-features --all-targets
	$(MAKE) check-web

check-core:
	mise exec -- cargo test --no-default-features

check-web:
	node web/download.test.mjs
	node --test web/camera.test.mjs web/clipboard.test.mjs \
		web/progress.test.mjs
	node tools/publish-release.test.mjs
	node tools/cache.test.mjs

check-container:
	docker build --load -t fastfill-checks .
	docker run --rm fastfill-checks

run:
	mise exec -- cargo run

web:
	mise run web

fmt:
	mise exec -- cargo fmt --all
	mise exec -- mdformat README.md docs

sample-form:
	mkdir -p docs/samples
	uv run --no-project python tools/generate_sample_form.py \
		docs/samples/fastfill-camera-sample-form.pdf
