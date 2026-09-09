#!/usr/bin/env bash
#
# Run the shipped uninstall hook against a throwaway tree, in every combination of its guards.
#
#     bash BUILD/crates/epoch-tauri/hooktest/run.sh
#
# Needs the `makensis` Tauri already downloads (`%LOCALAPPDATA%\tauri\NSIS`), or any other one
# on the PATH. Windows only, because the hook is.
#
# What it proves, and what it does not: the macro's body -- both guards, the shell variable
# context, the recursion. That a ticked checkbox reaches it is the generated `installer.nsi`'s
# job, verified by reading it (`BM_GETCHECK` in `un.ConfirmLeave`, the macro inserted inside
# `Section Uninstall` after Tauri's own block, both variables declared at file scope).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
shipped="$here/../installer-hooks.nsh"
work="${TMPDIR:-/tmp}/epoch-hooktest.$$"
mkdir -p "$work"
trap 'rm -rf "$work"' EXIT

makensis="${MAKENSIS:-$LOCALAPPDATA/tauri/NSIS/makensis.exe}"
[ -x "$makensis" ] || makensis="$(command -v makensis)"

# **One token, and it is printed.** A test that quietly edits the thing it is testing is not a
# test of that thing.
sed 's|"\$APPDATA\\Epoch"|"$%EPOCH_TEST_ROOT%\\Epoch"|' "$shipped" > "$work/hooks-under-test.nsh"
echo "the only difference from the shipped hook:"
diff "$shipped" "$work/hooks-under-test.nsh" | sed 's/^/    /' || true
[ "$(diff "$shipped" "$work/hooks-under-test.nsh" | grep -c '^[<>]')" = "2" ] ||
  { echo "expected exactly one changed line"; exit 1; }
cp "$here/hooktest.nsi" "$work/"

printf '\n%-9s %-9s  %-10s %s\n' checked updating "should" "happened"
fail=0
for pair in "1 0" "0 0" "1 1" "0 1"; do
  set -- $pair
  checked=$1 updating=$2
  tree="$work/case-$checked-$updating"
  mkdir -p "$tree/Epoch/library/models" "$tree/Epoch/vault"
  echo x > "$tree/Epoch/library/models/pretend.gguf"
  echo x > "$tree/Epoch/vault/mage.toml"

  # Compiled per case: the root is a compile-time expansion, which is what keeps a real
  # `%APPDATA%` unreachable from this binary.
  EPOCH_TEST_ROOT="$(cygpath -w "$tree" 2>/dev/null || echo "$tree")" \
    "$makensis" "$work/hooktest.nsi" > "$work/make.log" 2>&1
  EPOCH_TEST_CHECKED=$checked EPOCH_TEST_UPDATING=$updating "$work/hooktest.exe"

  saw="$(cat "$work/saw.out")"
  case "$saw" in
    "checked=$checked updating=$updating"*) ;;
    # The harness has to prove it read its arguments. A run that deleted nothing because it
    # never looked is indistinguishable from a guard doing its job.
    *) echo "the harness saw '$saw', not what it was given"; exit 1 ;;
  esac

  if [ "$checked" = "1" ] && [ "$updating" = "0" ]; then want=deleted; else want=kept; fi
  if [ -d "$tree/Epoch" ]; then got="kept $(find "$tree/Epoch" -type f | wc -l | tr -d ' ')"; else got=deleted; fi
  case "$got" in "$want"*) mark="" ;; *) mark="   <-- WRONG"; fail=1 ;; esac
  printf '%-9s %-9s  %-10s %s%s\n' "$checked" "$updating" "$want" "$got" "$mark"
done

[ "$fail" = "0" ] || exit 1
echo
echo "the hook deletes only when the box was ticked and this is not an update."
