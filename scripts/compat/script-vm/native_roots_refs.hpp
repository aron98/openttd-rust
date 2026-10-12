// Explicit RefTable operations are the experiment, never passive observation owners.
namespace native_roots {
static bool ref_table(HSQUIRRELVM vm,bool with_string) {
 if(!primary(vm,true))return false;
 std::array<HSQOBJECT,9> held;
 for(auto &slot:held)sq_resetobject(&slot);
 HSQOBJECT string;sq_resetobject(&string);
 sq_newarray(vm,0);
 for(size_t i=0;i<4;++i) {
  SQObjectPtr table=SQTable::Create(_ss(vm),0);
  if(!owner_trace::add(_table(table),char('A'+i)))return false;
  _array(stack_get(vm,-1))->Append(table);held[i]=table;sq_addref(vm,&held[i]);
  if(i==0 && with_string) {
   sq_pushstring(vm,"ref-slot");string=stack_get(vm,-1);
   if(!owner_trace::add(_string(string),'S'))return false;
   sq_addref(vm,&string);sq_pop(vm,1);
  }
 }
 sq_pop(vm,1);snapshot(vm,"tokens");
 for(size_t i=0;i<4;++i)object(std::string_view("ABCD").substr(i,1),held[i]);
 sq_addref(vm,&held[0]);object("repeat",held[0]);
 const bool removed=sq_release(vm,&held[0]);
 std::cout<<"roots release intermediate "<<removed<<'\n';object("remaining",held[0]);
 collect(vm,"retained-tokens");
 const bool last=sq_release(vm,&held[1]);sq_resetobject(&held[1]);
 std::cout<<"roots release last "<<last<<'\n';owner_trace::emit();
 for(size_t i=4;i<held.size();++i) {
  SQObjectPtr table=SQTable::Create(_ss(vm),0);
  if(!owner_trace::add(_table(table),char('A'+i)))return false;
  held[i]=table;sq_addref(vm,&held[i]);
 }
 collect(vm,"reused-grown");
 for(auto &slot:held)if(type(slot)!=OT_NULL){sq_release(vm,&slot);sq_resetobject(&slot);}
 if(with_string){sq_release(vm,&string);sq_resetobject(&string);}
 owner_trace::emit();collect(vm,"tokens-released");return true;
}
static int session(int argc,char **argv) {
 if(argc!=4)return 64;
 const std::string_view mode=argv[2],record=argv[3];
 if(record!="on" && record!="off")return 64;
 if(mode!="shared" && mode!="idle" && mode!="persistent" && mode!="replace" && mode!="same" &&
  mode!="friend-stack" && mode!="friend-local" && mode!="friend-host" && mode!="refs" && mode!="refs-string")return 64;
 owner_trace::begin(record=="on");auto vm=sq_open(256);bool ok=false;
 if(mode=="shared")ok=shared(vm);
 if(mode=="idle"){ok=primary(vm,true);if(ok){snapshot(vm,"idle");collect(vm,"idle-collected");}}
 if(mode=="persistent")ok=persistent(vm);
 if(mode=="replace" || mode=="same")ok=replacement(vm,mode=="same");
 if(mode=="friend-stack" || mode=="friend-local" || mode=="friend-host")ok=friend_context(vm,mode);
 if(mode=="refs" || mode=="refs-string")ok=ref_table(vm,mode=="refs-string");
 sq_close(vm);return owner_trace::finish() && ok?0:65;
}
}
