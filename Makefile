.PHONY: assets check check-xaml preview update-goldens

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

preview: check
	$(RUN) \
		-v "$(ROOT)/artifacts:/work/artifacts" \
		$(IMAGE)
	@echo "Preview files: artifacts/preview"

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
