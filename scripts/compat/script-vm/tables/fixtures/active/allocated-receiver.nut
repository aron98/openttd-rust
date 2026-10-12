::__owner_setup[0].f <- function(){return 1;};
local result=1+(2+(3+(4+(5+::__owner_setup[0].f()))));
::__owner_setup=null;
::print("collect-active");
return result==16 ? 1 : 0;
