.PHONY: build run test clean release help

build:
	@echo "Building project..."
	@cargo build

run: build
	@echo "Running project..."
	@cargo run

test:
	@echo "Running tests..."
	@cargo test

clean:
	@echo "Cleaning project..."
	@cargo clean

release:
	@echo "Building release..."
	@cargo build --release

## help: Показать список команд
help:
	@echo "BlockKick - Makefile commands:"
	@echo ""
	@echo "  make build    - Скомпилировать проект"
	@echo "  make run      - Запустить проект"
	@echo "  make test     - Запустить тесты"
	@echo "  make clean    - Очистить артефакты сборки"
	@echo "  make release  - Скомпилировать релизную версию"
	@echo "  make help     - Показать эту справку"
