# Run only on the target Linux by the main hardware session.
# Layout copied from the v5 read-profile.sh; 32 scratch words include all v6 results.
export PATH=/bin
set -e
echo DDR_INSPECT_BEGIN
trap 'rc=$?; echo PROFILE_SCRIPT_EXIT=$rc; echo DDR_INSPECT_END' EXIT
echo PROFILE_READ_BEGIN
i=0
while [ "$i" -lt 64 ]; do
  v=$(busybox devmem $((0x1f0040f700+i*4)) 32)
  echo "PRF1_WORD $i $v"
  i=$((i+1))
done
i=0
while [ "$i" -lt 32 ]; do
  v=$(busybox devmem $((0x1f0040f600+i*4)) 32)
  echo "PRF1_SCRATCH $i $v"
  i=$((i+1))
done
i=0
while [ "$i" -lt 32 ]; do
  v=$(busybox devmem $((0x1f0040ba00+i*4)) 32)
  echo "PROC1_WORD $i $v"
  i=$((i+1))
done
for tag in RTOS_A RTOS_B; do
  i=0
  while [ "$i" -lt 32 ]; do
    v=$(busybox devmem $((0x1f0040f800+i*4)) 32)
    echo "$tag $i $v"
    i=$((i+1))
  done
  if [ "$tag" = RTOS_A ]; then busybox sleep 1; fi
done
echo PROFILE_READ_END
