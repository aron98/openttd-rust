local n=0; foreach(v in [1,2,3]) { switch(v) { case 2:continue; default:n+=v;break; } }
return n;