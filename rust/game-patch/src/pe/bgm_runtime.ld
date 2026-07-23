SECTIONS
{
  . = 0x004d8000;
  .bgm : SUBALIGN(4)
  {
    *(.text*)
    *(.rdata*)
    *(.data*)
  }
  /DISCARD/ : { *(.eh_frame*) *(.comment*) *(.idata*) }
}
