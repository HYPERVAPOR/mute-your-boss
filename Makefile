.PHONY: all build test fmt clean proto run-core run-gateway install-tools

# Toolchain versions
PROTOC_VERSION := 27.3
PROTOC_GEN_GO_VERSION := v1.34.2
PROTOC_GEN_GO_GRPC_VERSION := v1.5.1

PROTOC := $(CURDIR)/tools/protoc/bin/protoc
GOPATH := $(shell go env GOPATH)
GO_BIN := $(GOPATH)/bin

all: build

build: proto
	PROTOC=$(PROTOC) cargo build --release
	cd gateway && go build -o bin/gateway .

test:
	PROTOC=$(PROTOC) cargo test
	cd gateway && go test ./...

fmt:
	cargo fmt
	cd gateway && go fmt ./...

install-tools:
	@echo "Installing protoc $(PROTOC_VERSION)..."
	@mkdir -p tools
	@curl -L -o tools/protoc.zip https://github.com/protocolbuffers/protobuf/releases/download/v$(PROTOC_VERSION)/protoc-$(PROTOC_VERSION)-linux-x86_64.zip
	@unzip -o tools/protoc.zip -d tools/protoc
	@rm tools/protoc.zip
	@echo "Installing Go protoc plugins..."
	@go install google.golang.org/protobuf/cmd/protoc-gen-go@$(PROTOC_GEN_GO_VERSION)
	@go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@$(PROTOC_GEN_GO_GRPC_VERSION)

proto:
	@mkdir -p gateway/internal/pb
	@PATH="$(GOPATH)/bin:$(PATH)" $(PROTOC) \
		--proto_path=proto \
		--go_out=gateway \
		--go_opt=module=github.com/HYPERVAPOR/mute-your-boss/gateway \
		--go-grpc_out=gateway \
		--go-grpc_opt=module=github.com/HYPERVAPOR/mute-your-boss/gateway \
		proto/myb.proto

run-core:
	PROTOC=$(PROTOC) cargo run --release -p myb-core

run-gateway:
	cd gateway && go run .

clean:
	cargo clean
	rm -rf gateway/bin
