def tile($i): {
  type:.chunks.MAPT.bytes[$i],height:.chunks.MAPH.bytes[$i],
  m1:.chunks.MAPO.bytes[$i],m2:(.chunks.MAP2.bytes[$i*2]*256+.chunks.MAP2.bytes[$i*2+1]),
  m3:.chunks.M3LO.bytes[$i],m4:.chunks.M3HI.bytes[$i],m5:.chunks.MAP5.bytes[$i],
  m6:.chunks.MAPE.bytes[$i],m7:.chunks.MAP7.bytes[$i],m8:(.chunks.MAP8.bytes[$i*2]*256+.chunks.MAP8.bytes[$i*2+1])
};
