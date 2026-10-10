local n=0; foreach(v in [1,2,3,4]) { if(v==2) continue; if(v==4) break; n+=v; }
return n;