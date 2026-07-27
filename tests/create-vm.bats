#!/usr/bin/env bats
# create-vm.sh: argument handling, content-based input validation, and
# dry-run command construction. Dry runs need no tart install, so these run
# everywhere. Inputs are fabricated from magic bytes -- validation must be
# by content, not extension.

setup() {
  cd "$BATS_TEST_DIRNAME/.." || exit 1
}

# A minimal "raw disk": GPT signature at LBA 1 (offset 512).
make_fake_raw() {
  dd if=/dev/zero of="$1" bs=1024 count=40 2>/dev/null
  printf 'EFI PART' | dd of="$1" bs=1 seek=512 conv=notrunc 2>/dev/null
}

# A minimal "installer ISO": ISO9660 signature -- and a GPT too, as hybrid
# ISOs really have, so the ISO check must win.
make_fake_iso() {
  dd if=/dev/zero of="$1" bs=1024 count=40 2>/dev/null
  printf 'EFI PART' | dd of="$1" bs=1 seek=512 conv=notrunc 2>/dev/null
  printf 'CD001' | dd of="$1" bs=1 seek=32769 conv=notrunc 2>/dev/null
}

# A minimal "qcow2": its 4-byte magic.
make_fake_qcow2() {
  dd if=/dev/zero of="$1" bs=1024 count=40 2>/dev/null
  printf 'QFI\xfb' | dd of="$1" bs=1 conv=notrunc 2>/dev/null
}

@test "create-vm.sh -h exits 0 and prints usage" {
  run ./bin/create-vm.sh -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "create-vm.sh requires a disk (-d)" {
  run ./bin/create-vm.sh -n
  [ "$status" -ne 0 ]
}

@test "create-vm.sh rejects an unknown option" {
  run ./bin/create-vm.sh -z
  [ "$status" -ne 0 ]
}

@test "create-vm.sh errors when the given disk is missing" {
  run ./bin/create-vm.sh -n -d /no/such/disk.raw
  [ "$status" -ne 0 ]
  [[ "$output" == *"not found"* ]]
}

@test "create-vm.sh rejects an installer ISO, even a hybrid one with a GPT" {
  make_fake_iso "$BATS_TEST_TMPDIR/fake.iso"
  run ./bin/create-vm.sh -n -d "$BATS_TEST_TMPDIR/fake.iso"
  [ "$status" -ne 0 ]
  [[ "$output" == *"ISO"* ]]
}

@test "create-vm.sh rejects a file that is neither raw disk nor qcow2" {
  echo "not a disk" >"$BATS_TEST_TMPDIR/junk.raw"
  run ./bin/create-vm.sh -n -d "$BATS_TEST_TMPDIR/junk.raw"
  [ "$status" -ne 0 ]
  [[ "$output" == *"neither"* ]]
}

@test "dry-run imports a raw disk: delete, create, swap, set" {
  make_fake_raw "$BATS_TEST_TMPDIR/disk.raw"
  run ./bin/create-vm.sh -n -d "$BATS_TEST_TMPDIR/disk.raw" -N testvm
  [ "$status" -eq 0 ]
  [[ "$output" == *"tart delete testvm"* ]]
  [[ "$output" == *"tart create --linux testvm"* ]]
  [[ "$output" == *"disk.img"* ]]
  [[ "$output" == *"--display-refit"* ]]
}

@test "dry-run detects qcow2 by magic and converts, even without the extension" {
  make_fake_qcow2 "$BATS_TEST_TMPDIR/mislabeled.img"
  run ./bin/create-vm.sh -n -d "$BATS_TEST_TMPDIR/mislabeled.img"
  [ "$status" -eq 0 ]
  [[ "$output" == *"qemu-img"* ]]
  [[ "$output" == *"convert"* ]]
}

@test "validation happens before the destructive delete" {
  # A rejected input must never print (or run) tart delete.
  make_fake_iso "$BATS_TEST_TMPDIR/fake.iso"
  run ./bin/create-vm.sh -n -d "$BATS_TEST_TMPDIR/fake.iso"
  [[ "$output" != *"tart delete"* ]]
}
