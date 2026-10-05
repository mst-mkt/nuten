import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      format: {
        command: "treefmt",
        cache: false,
      },
      "format:check": {
        command: "treefmt --ci",
        env: ["PATH"],
      },

      "lint:rust": {
        command: [
          "cargo clippy --workspace --all-targets -- --deny warnings",
          "RUSTDOCFLAGS='--deny warnings' cargo doc --workspace --no-deps",
          "cargo clippy --manifest-path editors/zed/Cargo.toml --target wasm32-wasip2 --all-targets -- --deny warnings",
        ],
        input: [{ auto: true }, "!target/**", "!editors/zed/target/**"],
        env: ["PATH"],
        untrackedEnv: ["NIX_*", "SDKROOT"],
        output: [],
      },
      "lint:ts": {
        command: "vp check",
      },

      "test:rust": {
        command: "cargo nextest run --workspace",
        input: ["Cargo.toml", "Cargo.lock", ".cargo/**", "crates/**", "tasks/oracle/**"],
        env: ["PATH", "NUTEN_CORPUS"],
        untrackedEnv: ["NIX_*", "SDKROOT"],
        output: [],
      },
      "test:oracle": {
        // Vite+ sets FORCE_COLOR=1, which makes nu color the error messages the oracle compares.
        command: 'env -u FORCE_COLOR cargo oracle format tasks/oracle/test_data "$NUTEN_CORPUS"',
        input: [{ auto: true }, "!target/**"],
        env: ["PATH", "NUTEN_CORPUS"],
        untrackedEnv: ["NIX_*", "SDKROOT"],
        output: [],
      },
      "test:zed": {
        command: "cargo test --manifest-path editors/zed/Cargo.toml",
        input: [{ auto: true }, "!editors/zed/target/**"],
        env: ["PATH"],
        untrackedEnv: ["NIX_*", "SDKROOT"],
        output: [],
      },
      "test:nu": {
        command: "nu scripts/test-nu.nu",
        input: [{ auto: true }, "!target/**"],
        env: ["PATH", "NU_LIB_DIRS"],
        untrackedEnv: ["NIX_*", "SDKROOT"],
        output: [],
      },
      "test:fuzz": {
        command: [
          "nix develop ./nix/dev#fuzz --command cargo fuzz run --fuzz-dir tasks/fuzz lex -- -max_total_time=60",
          "nix develop ./nix/dev#fuzz --command cargo fuzz run --fuzz-dir tasks/fuzz parse -- -max_total_time=60",
        ],
        cache: false,
      },
      "test:vscode": {
        command: "vp run nuten-vscode#test",
      },
    },
  },
});
