#!/usr/bin/env bats
# Bats tests for src/lib/utils.sh

setup() {
  load '../src/lib/utils.sh'
}

@test "trim removes surrounding whitespace" {
  result="$(trim '  hello  ')"
  [ "$result" = "hello" ]
}

@test "to_upper uppercases ASCII" {
  result="$(to_upper 'hello')"
  [ "$result" = "HELLO" ]
}
