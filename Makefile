.PHONY: assets check preview update-goldens

IMAGE := fastfill-checks
ROOT := $(CURDIR)
RUN := docker run --rm

check:
	docker build --load -t $(IMAGE) .
	$(RUN) $(IMAGE)

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
