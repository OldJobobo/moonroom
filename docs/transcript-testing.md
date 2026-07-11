# Transcript Testing Guide

Put `.transcript` files in a game's `tests/` directory. Each `>` block is one command and compares only that command's output.

```text
> take key
You take the brass key.
!flag took_key
!counter keys_taken 1

> north
Hall

A narrow hall.
!room hall
```

Directives are checked after the command and are not compared as prose:

```text
!contains text
!not_contains text
!room room_id
!scene scene_name | !scene none
!chapter chapter_name | !chapter none
!flag flag_name
!counter counter_name integer_value
```

Run all tests with `moonroom test path/to/game`. Use `--filter text` for one transcript and `--seed integer` to fix deterministic random output. `--update` refreshes expected prose while preserving directives; review the resulting diff before accepting it. `moonroom transcript path/to/game -o tests/recorded.transcript` is useful for capturing a first draft, then edit it into a focused regression test.
