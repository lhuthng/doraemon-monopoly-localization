# doraemon-patcher

Native Windows UI patcher for applying language patches to Doraemon Monopoly.

- Provides backup/restore for original game files.
- Applies language, music, DirectSound, game-speed, and graphics wrapper options.
- Shows concise hover help on the dashed underline beneath every option.
- Designed for local build and Windows verification only.

## Windows verification checklist

1. Copy `patcher.exe` beside a test `Doraemon.exe` on Windows.
2. Apply each language option and verify in-game text.
3. Test Restore backup functionality.
4. Test local music and cnc-ddraw wrapper options.
5. Enable 8-bit DirectSound and verify the game uses 22,050 Hz stereo,
   44,100 bytes/sec, 2-byte blocks, and 8-bit samples.
6. Enable local music, modern volume control, and 8-bit DirectSound together;
   verify menu music starts through the format-adaptive embedded runtime.
7. Pick a **Game speed** above Normal and verify in-game that the *normal*
   in-game speed setting animates proportionally faster while the *fast* one is
   unchanged. Set it back to Normal, reapply, and confirm the stock speed
   returns without needing a restore first.
