.PHONY: assets check preview update-goldens

IMAGE := fastfill-checks
ROOT := $(CURDIR)

check:
	docker build --load -t $(IMAGE) .
	docker run --rm \
		-v "$(ROOT)/artifacts:/work/artifacts" \
		$(IMAGE)

preview: check
	@echo "Preview files: artifacts/preview"

update-goldens:
	docker build --load -t $(IMAGE) .
	docker run --rm \
		-v "$(ROOT)/artifacts:/work/artifacts" \
		-v "$(ROOT)/tests:/work/tests" \
		$(IMAGE) --update-goldens

assets:
	docker build --load -t $(IMAGE) .
	docker run --rm \
		-v "$(ROOT)/src/FastFill.App/Assets:/assets" \
		$(IMAGE) --make-assets /assets
