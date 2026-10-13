// Passive configured-root sessions; no changes to native compiler or VM algorithms.
#include <limits>

std::string root_bytes(const std::string &hex) {
 if(hex=="-")return {};
 if(hex.size()%2!=0)throw std::runtime_error("odd hex bytes");
 auto nibble=[](char c)->unsigned {
  if(c>='0'&&c<='9')return static_cast<unsigned>(c-'0');
  if(c>='a'&&c<='f')return static_cast<unsigned>(c-'a'+10);
  throw std::runtime_error("invalid hex bytes");
 };
 std::string bytes;bytes.reserve(hex.size()/2);
 for(size_t n=0;n<hex.size();n+=2)bytes.push_back(static_cast<char>((nibble(hex[n])<<4)|nibble(hex[n+1])));
 return bytes;
}
SQInteger root_integer(const std::string &text) {
 size_t end=0;auto result=std::stoll(text,&end);
 if(end!=text.size())throw std::runtime_error("invalid integer");
 return result;
}
SQObjectPtr root_scalar(HSQUIRRELVM vm,const std::string &kind,const std::string &text) {
 if(kind=="string")return SQObjectPtr(SQString::Create(_ss(vm),root_bytes(text)));
 if(kind=="integer")return SQObjectPtr(root_integer(text));
 if(kind=="float") {
  auto bits=root_integer(text);
  if(bits<0||bits>UINT32_MAX)throw std::runtime_error("invalid f32 bits");
  return SQObjectPtr(std::bit_cast<float>(static_cast<uint32_t>(bits)));
 }
 if(kind=="bool") {
  if(text!="0"&&text!="1")throw std::runtime_error("invalid bool");
  return SQObjectPtr(text=="1");
 }
 if(kind=="null"&&text=="-")return SQObjectPtr();
 throw std::runtime_error("invalid scalar");
}
void root_empty(HSQUIRRELVM vm) {
 auto top=sq_gettop(vm);sq_newtable(vm);
 if(SQ_FAILED(sq_setroottable(vm)))throw std::runtime_error("install empty root failed");
 if(sq_gettop(vm)!=top)throw std::runtime_error("root installation changed stack");
}
void root_share(HSQUIRRELVM destination,HSQUIRRELVM source) {
 if(_ss(destination)!=_ss(source))throw std::runtime_error("cross-realm root sharing");
 auto top=sq_gettop(destination);sq_pushobject(destination,source->_roottable);
 if(SQ_FAILED(sq_setroottable(destination)))throw std::runtime_error("share root failed");
 if(sq_gettop(destination)!=top)throw std::runtime_error("root sharing changed stack");
}
void root_raw(HSQUIRRELVM vm,const std::string &hex) {
 SQObjectPtr key(SQString::Create(_ss(vm),root_bytes(hex))),found;
 std::cout<<"raw "<<hex<<' ';
 if(_table(vm->_roottable)->Get(key,found))value(found);else std::cout<<"absent\n";
}
void root_observation(HSQUIRRELVM vm,bool ok,const SQObjectPtr &closure,const SQObjectPtr &result,unsigned root_id) {
 auto proto=_funcproto(_closure(closure)->_function);
 if(!ok) {
  std::cout<<"runtime_error "<<vm->_ops_till_suspend<<' ';
  value(vm->_lasterror);
 } else if(!vm->_suspended) {std::cout<<"return "<<vm->_ops_till_suspend<<' ';value(result);}
 else {
  std::cout<<"suspend "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-proto->_instructions)<<'\n';
  for(SQInteger slot=0;slot<proto->_stacksize;++slot) {
   const auto &held=vm->GetAt(vm->_stackbase+slot);std::cout<<"frame "<<slot<<' ';
   if(type(held)==OT_TABLE&&_rawval(held)==_rawval(vm->_roottable))std::cout<<"root "<<root_id<<'\n';
   else value(held);
  }
 }
 std::cout<<"temp ";value(vm->temp_reg);
 // Same terminal host-stack cleanup as existing sliced(); does not clear temp_reg or retained result.
 if(!ok||!vm->_suspended)sq_settop(vm,0);
}
int root_slots_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream commands(argv[2]);if(!commands)return 66;
 auto primary=sq_open(256);auto independent=sq_open(256);int status=0;
 {
  auto child=sq_newthread(primary,256);SQObjectPtr child_owner=stack_get(primary,-1);sq_pop(primary,1);
  std::array<HSQUIRRELVM,3> vms={primary,child,independent};
  std::array<unsigned,3> root_ids={0,0,1};unsigned next_root=2;
  std::array<SQObjectPtr,32> closures;std::array<int,32> origins;origins.fill(-1);
  std::array<SQObjectPtr,3> results;std::array<int,3> active;active.fill(-1);
  try {
   root_empty(primary);root_share(child,primary);root_empty(independent);
   std::cout<<"configured_empty_roots 0 0 1\n";
   std::string line;size_t step=0;
   while(std::getline(commands,line)) {
    if(line.empty()||line[0]=='#')continue;
    std::istringstream command(line);std::string op;int realm;
    if(!(command>>op>>realm)||realm<0||realm>=3)throw std::runtime_error("invalid VM command");
    std::cout<<"step "<<step++<<' '<<line<<'\n';
    auto vm=vms[realm];
    if(op=="clear-result") {results[realm].Null();std::cout<<"result_released\n";}
    else {
     if(vm==nullptr)throw std::runtime_error("released VM");
     if(op=="compile") {
      size_t slot;std::string mode,path;
      if(vm->_suspended||!(command>>slot>>mode>>path)||slot>=closures.size()||origins[slot]!=-1)throw std::runtime_error("invalid compile slot");
      std::ifstream input(path,std::ios::binary);if(!input)throw std::runtime_error("missing source");
      std::string source((std::istreambuf_iterator<char>(input)),{});StringConsumer reader(source);SQRESULT outcome;
      if(mode=="buffer")outcome=sq_compilebuffer(vm,source,"root-slot-fixture",SQFalse);
      else if(mode=="unsigned"||mode=="utf8")outcome=sq_compile(vm,mode=="unsigned"?unsigned_feed:utf8_feed,&reader,"root-slot-fixture",SQFalse);
      else throw std::runtime_error("invalid feed");
      if(SQ_FAILED(outcome)) {std::cout<<"compile_error ";value(vm->_lasterror);}
      else {closures[slot]=stack_get(vm,-1);origins[slot]=realm;sq_pop(vm,1);std::cout<<"compiled\n";constant_dump(closures[slot]);}
     } else if(op=="raw") {
      std::string key;if(!(command>>key))throw std::runtime_error("invalid raw");root_raw(vm,key);
     } else if(op=="seed") {
      std::string key,kind,payload;if(!(command>>key>>kind>>payload))throw std::runtime_error("invalid seed");
      auto top=sq_gettop(vm);auto scalar=root_scalar(vm,kind,payload);
      sq_pushroottable(vm);sq_pushstring(vm,root_bytes(key));sq_pushobject(vm,scalar);
      auto result=sq_rawset(vm,-3);sq_settop(vm,top);
      if(SQ_FAILED(result))throw std::runtime_error("raw root seed failed");
      std::cout<<"seeded\n";
     } else if(op=="refs") {
      std::string payload;if(!(command>>payload))throw std::runtime_error("invalid refs");
      SQObjectPtr probe(SQString::Create(_ss(vm),root_bytes(payload)));
      std::cout<<"refs "<<payload<<' '<<(_string(probe)->_uiRef-1)<<'\n';
     } else if(op=="temp") {std::cout<<"temp ";value(vm->temp_reg);}
     else if(op=="root") {std::cout<<"root "<<root_ids[realm]<<'\n';}
     else if(op=="empty") {
      if(vm->_suspended)throw std::runtime_error("cannot replace suspended VM root");
      root_empty(vm);root_ids[realm]=next_root++;std::cout<<"root "<<root_ids[realm]<<'\n';
     } else if(op=="share") {
      int source;if(vm->_suspended||!(command>>source)||source<0||source>=3||vms[source]==nullptr)throw std::runtime_error("invalid shared root");
      root_share(vm,vms[source]);root_ids[realm]=root_ids[source];std::cout<<"root "<<root_ids[realm]<<'\n';
     } else if(op=="run") {
      size_t slot;std::string credit;
      if(vm->_suspended||!(command>>slot>>credit)||slot>=closures.size()||origins[slot]!=realm)throw std::runtime_error("invalid run");
      auto amount=root_integer(credit);if(amount<0)throw std::runtime_error("negative initial credit");
      results[realm].Null();active[realm]=static_cast<int>(slot);
      sq_pushroottable(vm);vm->_can_suspend=true;vm->_ops_till_suspend=amount;
      auto ok=vm->Call(closures[slot],1,vm->_top-1,results[realm],SQFalse,SQTrue);
      root_observation(vm,ok,closures[slot],results[realm],root_ids[realm]);
      if(!ok||!vm->_suspended)active[realm]=-1;
     } else if(op=="advance") {
      std::string credit;if(!(command>>credit))throw std::runtime_error("invalid advance");
      auto amount=root_integer(credit);
      if(amount<0||vm->_ops_till_suspend>std::numeric_limits<SQInteger>::max()-amount)throw std::runtime_error("invalid credit addition");
      if(active[realm]<0)std::cout<<"idle\n";
      else {
       vm->_ops_till_suspend+=amount;
       auto ok=vm->Execute(_null_,vm->_top,-1,-1,results[realm],SQFalse,SQVM::ET_RESUME_OPENTTD);
       root_observation(vm,ok,closures[active[realm]],results[realm],root_ids[realm]);
       if(!ok||!vm->_suspended)active[realm]=-1;
      }
     } else if(op=="drop") {
      size_t slot;if(!(command>>slot)||slot>=closures.size()||origins[slot]!=realm||active[realm]==static_cast<int>(slot))throw std::runtime_error("invalid drop");
      closures[slot].Null();origins[slot]=-1;std::cout<<"dropped\n";
     } else if(op=="release-child") {
      if(realm!=1||vm->_suspended)throw std::runtime_error("requires idle child");
      for(int origin:origins)if(origin==1)throw std::runtime_error("child closure still held");
      vms[1]=nullptr;child_owner.Null();std::cout<<"child_released\n";
     } else throw std::runtime_error("unknown command");
    }
    std::string extra;if(command>>extra)throw std::runtime_error("extra command fields");
   }
   for(auto vm:vms)if(vm!=nullptr&&vm->_suspended)throw std::runtime_error("session ended suspended");
  } catch(const std::exception &error) {std::cerr<<"observer_error "<<error.what()<<'\n';status=65;}
  for(auto &result:results)result.Null();
  for(auto &closure:closures)closure.Null();
  child_owner.Null();
 }
 sq_close(independent);sq_close(primary);return status;
}
