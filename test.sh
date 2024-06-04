#!/bin/bash
# run unit tests, integration tests, and display coverage for tests
cd "$(dirname $0)"

PROJECT_ROOT="$PWD"
OUTPUT_DIR="$PROJECT_ROOT/target/coverage"
PROFILE_FILE="ssgen.profraw"
MERGED_PROFILE_FILE="ssgen.profdata"

# ---- #

if [[ "$1" == "help" ]] || [[ "$1" == "" ]]; then
  echo "Usage: $0 <coverage|profile> [open]"
  echo "Generate coverage report or profile the program"
  exit
fi

fail() {
    echo "$@"
    exit 1
}

# prereqs
if [[ "$(which grcov; echo $?)" == "1" ]]; then
    echo "Please install grcov on your system"
    exit
fi

if [[ "$(which llvm-profdata; echo $?)" == "1" ]]; then
    echo "Please install llvm-tools on your system"
    exit
fi

if [[ "$(which samply; echo $?)" == "1" ]]; then
    echo "Please install samply on your system"
    exit
fi

# env vars
export LLVM_PROFILE_FILE="$OUTPUT_DIR/$PROFILE_FILE"
export RUSTFLAGS="-Cinstrument-coverage -Ccodegen-units=1 \
    -Copt-level=0 -Clink-dead-code -Coverflow_checks=off \
    "
export RUSTDOCFLAGS="-Cpanic=abort"
export CARGO_INCREMENTAL=0
export RUST_BACKTRACE=1

# begin
echo "Starting..."

# PWD becomes $OUTPUT DIR
mkdir -p "$OUTPUT_DIR" && cd "$OUTPUT_DIR" && (rm -rv "$PWD/*" 2> /dev/null ;true) || fail "directory error!"

echo "Building executable..."
cargo build || fail "build error!"

if [[ "$1" == "coverage" ]]; then
  echo "Running tests..."
  cargo test -- --test-threads=1 || fail "unit testing error!"

  echo "Merging profdata..."
  llvm-profdata merge --sparse -o "$MERGED_PROFILE_FILE" "$PROFILE_FILE" || fail "profdata error!"

  # args shared between show and report
  TEST_BIN="$(ls -Art $PROJECT_ROOT/target/debug/deps/ssgen* | tail -n 1)"
  LLVM_COV_ARGS="--use-color \
    --ignore-filename-regex=/.cargo/registry \
    --ignore-filename-regex=panic.rs \
    --ignore-filename-regex=fast_local.rs \
    --ignore-filename-regex=main.rs \
    --instr-profile=$OUTPUT_DIR/$MERGED_PROFILE_FILE \
    --object $TEST_BIN \
  "

  echo "Showing coverage..."
  llvm-cov show \
    $LLVM_COV_ARGS \
    --show-instantiations --show-line-counts-or-regions \
    --Xdemangler=rust-demangler \
    || fail "llvm-cov show error!"

  echo "Creating report..."
  llvm-cov report $LLVM_COV_ARGS || fail "llvm-cov report error!"

  echo "Generating HTML report..."
  grcov "$OUTPUT_DIR/$PROFILE_FILE" \
    --binary-path "$TEST_BIN" \
    -s "$PROJECT_ROOT" \
    -t html \
    -o "$OUTPUT_DIR" \
    --ignore src/main.rs \
    --llvm-path "$(dirname $(which llvm-profdata))" \
    || fail "grcov error!"

  if [[ "$2" != "" ]]; then
    echo "Opening report in browser..."
    xdg-open "$OUTPUT_DIR/html/index.html" || fail "xdg-open error!"
  fi
fi # end coverage

if [[ "$1" == "profile" ]]; then
  echo "Profiling full example..."
  FULL_EXAMPLE="$OUTPUT_DIR/example_full/"
  EXECUTABLE="$PROJECT_ROOT/target/debug/ssgen"
  mkdir -p "$FULL_EXAMPLE" || fail "direcory error"
  touch "$FULL_EXAMPLE"/test.txt
  samply record -- "$EXECUTABLE" --debug --output "$FULL_EXAMPLE/test.txt" --input "$PROJECT_ROOT/examples/full/" || fail "samply/ssgen error!"
fi # end profile

echo "Done!"
