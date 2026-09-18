.PHONY: assets check check-xaml clean lab preview sample-form update-goldens

IMAGE := fastfill-checks
ROOT := $(CURDIR)
RUN := docker run --rm
XAML_EVENT_BREAK := =\r?\n[[:space:]]*"[A-Za-z][A-Za-z0-9_]*_[A-Za-z0-9_]+"

check: check-xaml
	docker build --load -t $(IMAGE) .
	$(RUN) $(IMAGE)

check-xaml:
	@if rg -n -U '$(XAML_EVENT_BREAK)' \
		src/FastFill.App -g '*.xaml'; then \
		echo "XAML event values must remain on one line."; \
		exit 1; \
	fi

clean:
	find src -type d \( -name bin -o -name obj \) -prune \
		-exec rm -rf {} +
	rm -rf artifacts/check-workspace \
		artifacts/preview \
		artifacts/scan-analysis \
		artifacts/unsafe-workspace \
		artifacts/unsafe.docscan

preview: check
	$(RUN) \
		-v "$(ROOT)/artifacts:/work/artifacts" \
		$(IMAGE)
	@echo "Preview files: artifacts/preview"

lab:
	docker build --load -t $(IMAGE) .
	docker run --rm --init --network host \
		$(IMAGE) --lab http://127.0.0.1:5077

update-goldens:
	docker build --load -t $(IMAGE) .
	$(RUN) \
		-v "$(ROOT)/artifacts:/work/artifacts" \
		-v "$(ROOT)/tests:/work/tests" \
		$(IMAGE) --update-goldens

assets:
	docker build --load -t $(IMAGE) .
	$(RUN) \
		-v "$(ROOT)/src/FastFill.App/Assets:/assets" \
		$(IMAGE) --make-assets /assets

sample-form:
	mkdir -p docs/samples
	uv run --no-project python tools/generate_sample_form.py \
		docs/samples/fastfill-camera-sample-form.pdf
