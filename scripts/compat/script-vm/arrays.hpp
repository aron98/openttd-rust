// Array host sessions execute the unchanged native compiler and VM.
#include "src/3rdparty/squirrel/squirrel/sqarray.h"
#include <vector>

struct ArrayHeld {
 HSQOBJECT object;
 HSQUIRRELVM owner=nullptr;
 ArrayHeld(){sq_resetobject(&object);}
 ArrayHeld(const ArrayHeld &)=delete;
 ArrayHeld &operator=(const ArrayHeld &)=delete;
 ~ArrayHeld(){clear();}
 void clear(){if(owner!=nullptr)sq_release(owner,&object);owner=nullptr;sq_resetobject(&object);}
 void capture(HSQUIRRELVM vm,SQInteger index,HSQUIRRELVM lifetime_owner){
  if(_ss(vm)!=_ss(lifetime_owner))throw std::runtime_error("cross-realm external owner");
  HSQOBJECT next;if(SQ_FAILED(sq_getstackobj(vm,index,&next)))throw std::runtime_error("missing stack object");
  sq_addref(lifetime_owner,&next);clear();object=next;owner=lifetime_owner;
 }
 bool present()const{return owner!=nullptr;}
};
struct ArrayStack {
 HSQUIRRELVM vm;SQInteger top;
 explicit ArrayStack(HSQUIRRELVM v):vm(v),top(sq_gettop(v)){}
 ~ArrayStack(){sq_settop(vm,top);}
};
struct ArrayLabels {
 std::array<ArrayHeld,256> watches;
 size_t used=0;
 size_t label(HSQUIRRELVM vm,const SQObject &object,HSQUIRRELVM lifetime_owner){
  if(type(object)!=OT_ARRAY)throw std::runtime_error("label requires array");
  for(size_t i=0;i<used;++i){
   const auto &old=_weakref(watches[i].object)->_obj;
   if(watches[i].owner==lifetime_owner&&type(old)==OT_ARRAY&&_rawval(old)==_rawval(object))return i+1;
  }
  if(used==watches.size())throw std::runtime_error("array label limit");
  ArrayStack stack(vm);sq_pushobject(vm,object);sq_weakref(vm,-1);
  watches[used].capture(vm,-1,lifetime_owner);return ++used;
 }
 void print(HSQUIRRELVM vm,const SQObject &object,HSQUIRRELVM lifetime_owner){
  if(type(object)==OT_ARRAY){
   const auto id=label(vm,object,lifetime_owner);
   ArrayStack stack(vm);sq_pushobject(vm,object);
   std::cout<<"array "<<id<<" length "<<sq_getsize(vm,-1)<<'\n';
  }else value(SQObjectPtr(object));
 }
 void snapshot(){
  for(size_t i=0;i<used;++i){
   auto vm=watches[i].owner;
   // Raw borrowed identity, not an undeclared strong probe owner.
   const SQObject object=_weakref(watches[i].object)->_obj;
   if(type(object)==OT_NULL){std::cout<<"array_dead "<<i+1<<'\n';continue;}
   {
    ArrayStack stack(vm);sq_pushobject(vm,object);
    auto length=sq_getsize(vm,-1);
    std::cout<<"array_state "<<i+1<<" length "<<length<<'\n';
    for(SQInteger index=0;index<length;++index){
     sq_pushinteger(vm,index);
     if(SQ_FAILED(sq_rawget(vm,-2)))throw std::runtime_error("array snapshot index");
     std::cout<<"element "<<i+1<<' '<<index<<' ';
     print(vm,stack_get(vm,-1),vm);sq_pop(vm,1);
    }
   }
   std::cout<<"array_refs "<<i+1<<' '<<_array(object)->_uiRef<<'\n';
  }
 }
};
size_t array_slot(std::istringstream &command,size_t size){
 std::string token;if(!(command>>token))throw std::runtime_error("missing slot");
 auto number=root_integer(token);
 if(number<0||static_cast<SQUnsignedInteger>(number)>=size)throw std::runtime_error("slot range");
 return static_cast<size_t>(number);
}
std::string array_word(std::istringstream &command){
 std::string result;if(!(command>>result))throw std::runtime_error("missing command argument");return result;
}
void array_end(std::istringstream &command){
 std::string extra;if(command>>extra)throw std::runtime_error("extra command fields");
}
void array_require(bool condition,const char *message){if(!condition)throw std::runtime_error(message);}

int arrays_session(int argc,char **argv){
 if(argc!=3)return 64;
 std::ifstream commands(argv[2]);if(!commands)return 66;
 auto primary=sq_open(256);auto independent=sq_open(256);int status=0;
 {
  ArrayHeld child_owner;
  auto child=sq_newthread(primary,256);child_owner.capture(primary,-1,primary);sq_pop(primary,1);
  std::array<HSQUIRRELVM,3> vms={primary,child,independent};
  std::array<HSQUIRRELVM,3> owners={primary,primary,independent};
  std::array<unsigned,3> root_ids={0,0,1};unsigned next_root=2;
  std::array<ArrayHeld,32> programs;
  std::array<int,32> origins;origins.fill(-1);
  std::array<ArrayHeld,3> results;
  std::array<ArrayHeld,32> handles;
  std::array<int,3> active;active.fill(-1);
  ArrayLabels labels;
  auto observe=[&](size_t which,bool ok,const SQObjectPtr &result){
   auto vm=vms[which];auto owner=owners[which];
   auto slot=static_cast<size_t>(active[which]);
   auto proto=_funcproto(_closure(programs[slot].object)->_function);
   if(!ok){std::cout<<"runtime_error "<<vm->_ops_till_suspend<<' ';labels.print(vm,vm->_lasterror,owner);}
   else if(!vm->_suspended){
    {ArrayStack stack(vm);sq_pushobject(vm,result);results[which].capture(vm,-1,owner);}
    std::cout<<"return "<<vm->_ops_till_suspend<<' ';labels.print(vm,result,owner);
   }else{
    std::cout<<"suspend "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-proto->_instructions)<<'\n';
    for(SQInteger n=0;n<proto->_stacksize;++n){
     const auto &held=vm->GetAt(vm->_stackbase+n);std::cout<<"frame "<<n<<' ';
     if(type(held)==OT_TABLE&&_rawval(held)==_rawval(vm->_roottable))std::cout<<"root "<<root_ids[which]<<'\n';
     else labels.print(vm,held,owner);
    }
   }
   std::cout<<"temp ";labels.print(vm,vm->temp_reg,owner);
   if(!ok||!vm->_suspended){sq_settop(vm,0);active[which]=-1;}
  };
  try{
   root_empty(primary);root_share(child,primary);root_empty(independent);
   std::cout<<"configured_empty_roots 0 0 1\n";
   std::string line;size_t step=0;
   while(std::getline(commands,line)){
    if(line.empty()||line[0]=='#')continue;
    std::istringstream command(line);auto op=array_word(command);auto which=array_slot(command,vms.size());
    auto vm=vms[which];auto owner=owners[which];
    array_require(vm!=nullptr,"released VM");
    std::cout<<"step "<<step++<<' '<<line<<'\n';
    auto before=sq_gettop(vm);
    bool execution=false;
    if(op=="compile"){
     auto slot=array_slot(command,programs.size());auto mode=array_word(command);auto path=array_word(command);array_end(command);
     array_require(!vm->_suspended&&!programs[slot].present(),"invalid compile slot/state");
     ArrayStack stack(vm);std::ifstream input(path,std::ios::binary);array_require(bool(input),"missing source");
     std::string source((std::istreambuf_iterator<char>(input)),{});StringConsumer reader(source);SQRESULT outcome;
     if(mode=="buffer")outcome=sq_compilebuffer(vm,source,"array-fixture",SQFalse);
     else if(mode=="unsigned"||mode=="utf8")outcome=sq_compile(vm,mode=="unsigned"?unsigned_feed:utf8_feed,&reader,"array-fixture",SQFalse);
     else throw std::runtime_error("invalid feed");
     if(SQ_FAILED(outcome)){std::cout<<"compile_error ";labels.print(vm,vm->_lasterror,owner);}
     else{programs[slot].capture(vm,-1,owner);origins[slot]=static_cast<int>(which);std::cout<<"compiled\n";constant_dump(SQObjectPtr(programs[slot].object));}
    }else if(op=="run"||op=="advance"){
     execution=true;size_t slot;
     if(op=="run")slot=array_slot(command,programs.size());
     else{array_require(active[which]>=0,"advance requires active frame");slot=static_cast<size_t>(active[which]);}
     auto amount=root_integer(array_word(command));array_end(command);array_require(amount>=0,"negative credit");
     SQObjectPtr result;bool ok;
     if(op=="run"){
      array_require(!vm->_suspended&&origins[slot]==static_cast<int>(which),"invalid run slot/state");
      results[which].clear();active[which]=static_cast<int>(slot);
      sq_pushroottable(vm);vm->_can_suspend=true;vm->_ops_till_suspend=amount;
      SQObjectPtr closure(programs[slot].object);
      ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
     }else{
      array_require(vm->_ops_till_suspend<=std::numeric_limits<SQInteger>::max()-amount,"credit overflow");
      vm->_ops_till_suspend+=amount;
      ok=vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
     }
     observe(which,ok,result);
    }else if(op=="drop"){
     auto slot=array_slot(command,programs.size());array_end(command);
     array_require(origins[slot]==static_cast<int>(which)&&active[which]!=static_cast<int>(slot),"invalid program release");
     programs[slot].clear();origins[slot]=-1;std::cout<<"program_released\n";
    }else if(op=="clear-result"){
     array_end(command);results[which].clear();std::cout<<"result_released\n";
    }else if(op=="make-array"){
     auto slot=array_slot(command,handles.size());auto count=array_slot(command,257);
     array_require(!handles[slot].present(),"handle occupied");
     ArrayStack stack(vm);sq_newarray(vm,0);
     for(size_t n=0;n<count;++n){
      auto kind=array_word(command);auto payload=array_word(command);auto scalar=root_scalar(vm,kind,payload);
      sq_pushobject(vm,scalar);array_require(SQ_SUCCEEDED(sq_arrayappend(vm,-2)),"host array append failed");
     }
     array_end(command);handles[slot].capture(vm,-1,owner);
     std::cout<<"handle "<<slot<<' ';labels.print(vm,handles[slot].object,owner);
    }else if(op=="hold-result"||op=="hold-root"){
     auto slot=array_slot(command,handles.size());array_require(!handles[slot].present(),"handle occupied");
     ArrayStack stack(vm);
     if(op=="hold-result"){
      array_require(results[which].present(),"no result");sq_pushobject(vm,results[which].object);
     }else{
      auto key=root_bytes(array_word(command));sq_pushroottable(vm);sq_pushstring(vm,key);
      array_require(SQ_SUCCEEDED(sq_rawget(vm,-2)),"missing own root slot");
     }
     array_end(command);array_require(sq_gettype(vm,-1)==OT_ARRAY,"hold requires array");
     handles[slot].capture(vm,-1,owner);std::cout<<"handle "<<slot<<' ';labels.print(vm,handles[slot].object,owner);
    }else if(op=="drop-handle"){
     auto slot=array_slot(command,handles.size());array_end(command);
     array_require(handles[slot].present()&&handles[slot].owner==owner,"wrong handle realm");
     handles[slot].clear();std::cout<<"handle_released "<<slot<<'\n';
    }else if(op=="seed"||op=="root-store-array"){
     auto key=root_bytes(array_word(command));ArrayStack stack(vm);sq_pushroottable(vm);sq_pushstring(vm,key);
     if(op=="seed"){
      auto kind=array_word(command);auto payload=array_word(command);auto scalar=root_scalar(vm,kind,payload);sq_pushobject(vm,scalar);
     }else{
      auto slot=array_slot(command,handles.size());array_require(handles[slot].present()&&handles[slot].owner==owner,"foreign or absent array handle");
      sq_pushobject(vm,handles[slot].object);
     }
     array_end(command);array_require(SQ_SUCCEEDED(sq_rawset(vm,-3)),"host root insertion failed");std::cout<<"seeded\n";
    }else if(op=="raw"){
     auto key=root_bytes(array_word(command));array_end(command);ArrayStack stack(vm);sq_pushroottable(vm);sq_pushstring(vm,key);
     if(SQ_FAILED(sq_rawget(vm,-2)))std::cout<<"raw absent\n";
     else{std::cout<<"raw ";labels.print(vm,stack_get(vm,-1),owner);}
    }else if(op=="array-get"||op=="array-set"||op=="array-length"){
     auto slot=array_slot(command,handles.size());array_require(handles[slot].present()&&handles[slot].owner==owner,"wrong handle realm");
     ArrayStack stack(vm);sq_pushobject(vm,handles[slot].object);
     if(op=="array-length"){array_end(command);std::cout<<"length "<<sq_getsize(vm,-1)<<'\n';}
     else{
      auto index=root_integer(array_word(command));sq_pushinteger(vm,index);SQRESULT result;
      if(op=="array-get"){array_end(command);result=sq_rawget(vm,-2);}
      else{
       auto kind=array_word(command);auto payload=array_word(command);array_end(command);
       auto scalar=root_scalar(vm,kind,payload);sq_pushobject(vm,scalar);result=sq_rawset(vm,-3);
      }
      if(SQ_FAILED(result)){std::cout<<"host_error ";labels.print(vm,vm->_lasterror,owner);}
      else if(op=="array-get"){std::cout<<"get ";labels.print(vm,stack_get(vm,-1),owner);}
      else std::cout<<"set\n";
     }
    }else if(op=="same-object"){
     auto left=array_slot(command,handles.size());auto right=array_slot(command,handles.size());array_end(command);
     array_require(handles[left].present()&&handles[right].present()&&handles[left].owner==owner&&handles[right].owner==owner,"wrong handle realm");
     std::cout<<"same "<<(_rawval(handles[left].object)==_rawval(handles[right].object))<<'\n';
    }else if(op=="weak-state"){
     auto label=array_slot(command,labels.used+1);array_end(command);array_require(label>0,"labels start at one");
     auto &watch=labels.watches[label-1];array_require(watch.owner==owner,"wrong weak realm");
     ArrayStack stack(vm);sq_pushobject(vm,watch.object);array_require(SQ_SUCCEEDED(sq_getweakrefval(vm,-1)),"weak read failed");
     std::cout<<"weak "<<label<<' ';labels.print(vm,stack_get(vm,-1),owner);
    }else if(op=="temp"){array_end(command);std::cout<<"temp ";labels.print(vm,vm->temp_reg,owner);}
    else if(op=="refs"){
     auto payload=root_bytes(array_word(command));array_end(command);SQObjectPtr probe(SQString::Create(_ss(vm),payload));
     std::cout<<"string_refs "<<(_string(probe)->_uiRef-1)<<'\n';
    }else if(op=="empty"){
     array_end(command);array_require(!vm->_suspended,"root replacement requires idle VM");root_empty(vm);root_ids[which]=next_root++;std::cout<<"root "<<root_ids[which]<<'\n';
    }else if(op=="share"){
     auto from=array_slot(command,vms.size());array_end(command);
     array_require(!vm->_suspended&&vms[from]!=nullptr&&owners[from]==owner,"invalid root sharing");
     root_share(vm,vms[from]);root_ids[which]=root_ids[from];std::cout<<"root "<<root_ids[which]<<'\n';
    }else if(op=="collect"){
     array_end(command);for(auto candidate:vms)array_require(candidate==nullptr||!candidate->_suspended,"collect requires idle runners");
     std::cout<<"collected "<<sq_collectgarbage(owner)<<'\n';
    }else if(op=="release-child"){
     array_end(command);array_require(which==1&&!vm->_suspended,"release requires idle child");
     for(int origin:origins)array_require(origin!=1,"child program retained");
     array_require(!results[1].present(),"child result retained");
     vms[1]=nullptr;child_owner.clear();std::cout<<"child_released\n";
    }else throw std::runtime_error("unknown array command");
    if(vms[which]!=nullptr){
     auto after=sq_gettop(vms[which]);std::cout<<"stack_top "<<before<<' '<<after<<'\n';
     if(!execution)array_require(before==after,"host operation changed stack top");
    }
    std::cout<<"external_child "<<child_owner.present()<<'\n';
    for(size_t n=0;n<programs.size();++n)if(programs[n].present())std::cout<<"external_program "<<n<<" vm "<<origins[n]<<'\n';
    for(size_t n=0;n<results.size();++n)if(results[n].present())std::cout<<"external_result "<<n<<'\n';
    for(size_t n=0;n<handles.size();++n)if(handles[n].present())std::cout<<"external_handle "<<n<<" realm "<<(handles[n].owner==primary?0:2)<<'\n';
    for(size_t n=0;n<vms.size();++n)if(vms[n]!=nullptr)std::cout<<"vm_root "<<n<<' '<<root_ids[n]<<" active "<<active[n]<<'\n';
    labels.snapshot();
   }
   for(auto vm:vms)array_require(vm==nullptr||!vm->_suspended,"session ended suspended");
  }catch(const std::exception &error){std::cerr<<"observer_error "<<error.what()<<'\n';status=65;}
  // All external references and weak watches die before their shared-state owners.
 }
 sq_close(independent);sq_close(primary);return status;
}
