include "tile";
def absval: if . < 0 then -. else . end;
def height($x;$y): [4,([0,([($x-8|absval),($y-10|absval)]|max)-2]|max)]|min;
. as $world |
{schema_version:1,edits:[range(4;17) as $y|range(2;15) as $x|($y*64+$x) as $i|
 {kind:"tile",index:$i,value:($world|tile($i)|.height=height($x;$y)|
  if $x>=6 and $x<=9 and $y>=8 and $y<=11 then
   .type=96|.m1=17|.m2=0|.m3=1|.m4=0|.m5=0|.m6=0|.m7=0|.m8=0
  elif $i==650 then .m1=16|.m2=240
  else . end)}]}
