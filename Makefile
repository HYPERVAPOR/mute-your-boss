.PHONY: all build test fmt clean proto run-core run-gateway

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
