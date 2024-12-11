HOOKS=pre-commit

init: hooks install-bin

install-bin: cargo-run-bin
	cargo bin --install

hooks:
	@for hook in ${HOOKS}; do \
  		if [ ! -f ".git/hooks/$$hook" ]; then \
			echo "#!/bin/sh\nmake $$hook" > ".git/hooks/$$hook"; \
			chmod +x ".git/hooks/$$hook"; \
			echo "[OK] Hook installed: $$hook"; \
		fi \
	done

cargo-run-bin:
	@if ! which cargo-bin >/dev/null; then \
		read -p "cargo-run-bin is not installed. Install now? [y/N]: " sure && \
			case "$$sure" in \
				[yY]) \
				  	echo "cargo install cargo-run-bin"; \
					cargo install cargo-run-bin; \
					;; \
				*) \
					echo "Cannot proceed without cargo-run-bin."; \
					exit 1; \
					;; \
			esac \
	fi

test:
	cargo test --all-features -- --test-threads=1

lint:
	cargo fmt --check
	cargo clippy -- -Dwarnings

lint-fix:
	cargo fmt

test-watch:
	cargo bin cargo-watch -x "bin cargo-nextest run --all-features --no-fail-fast $T"

pre-commit: test lint
