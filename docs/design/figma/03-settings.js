const page=await figma.getNodeByIdAsync(STATE.pages.screens);await figma.setCurrentPageAsync(page);
const root=await figma.getNodeByIdAsync(STATE.wrappers.screens);
if(root.children.length){throw new Error("Review wrapper is already populated");}
const comps={};for(const [name,id] of Object.entries(STATE.components)){comps[name]=await figma.getNodeByIdAsync(id);}
const screenIds={},navigation=[],customLinks=[],backLinks=[];
await txt(root,"Design title","Mo 墨 · 更轻盈的输入体验","Display");
await txt(root,"Design description","Windows 10 桌面设计 · 常用设置 / 外观与皮肤 / 隐私与学习 / 自定义皮肤","Body","Paper","muted");
async function button(parent,label,style="Primary"){return inst(comps["Button/"+style],parent,{Label:label});}
async function setting(parent,title,description,type="Choice",value=""){
 const n=inst(comps["Row/"+type],parent,{Title:title,Description:description});
 if(type==="Choice"){
  const select=n.findAllWithCriteria({types:["INSTANCE"]}).find(i=>i.mainComponent.id===STATE.components.Select);
  const key=Object.keys(select.componentProperties).find(k=>k.startsWith("Value#"));
  select.setProperties({[key]:value});
 }
 if(type==="Toggle"&&value==="Off"){
  const control=n.findAllWithCriteria({types:["INSTANCE"]}).find(i=>i.mainComponent.id===STATE.components["Toggle/On"]);
  control.swapComponent(comps["Toggle/Off"]);
 }
 return n;
}
async function card(parent,title){
 const c=box(parent,title,"VERTICAL",840,24,8,"Paper","bg");round(c,12);c.strokes=[paint("Paper","border")];await txt(c,"Section heading",title,"Section");return c;
}
async function shell(parent,key,title,subtitle,selected){
 const w=box(parent,key,"VERTICAL",1100,0,0,"Paper","bg");w.resize(1100,760);w.primaryAxisSizingMode="FIXED";w.counterAxisSizingMode="FIXED";round(w,20);w.clipsContent=true;await w.setEffectStyleIdAsync(STATE.effects.Window);screenIds[key]=w.id;
 const titlebar=box(w,"Window title bar","HORIZONTAL",1100,0,0);titlebar.resize(1100,44);titlebar.primaryAxisSizingMode="FIXED";titlebar.counterAxisAlignItems="CENTER";titlebar.primaryAxisAlignItems="SPACE_BETWEEN";titlebar.paddingLeft=20;titlebar.paddingRight=16;
 await txt(titlebar,"App title","Mo 墨 · 设置","Caption","Paper","muted");
 const win=box(titlebar,"Window controls","HORIZONTAL",null,0,16);win.counterAxisAlignItems="CENTER";await txt(win,"Minimize","—","Caption","Paper","muted");await txt(win,"Maximize","□","Caption","Paper","muted");icon(win,"close","Paper",16);
 const body=box(w,"Window body","HORIZONTAL",1100,0,0);body.resize(1100,716);body.primaryAxisSizingMode="FIXED";
 const side=box(body,"Sidebar","VERTICAL",212,16,8,"Paper","canvas");side.resize(212,716);side.primaryAxisSizingMode="FIXED";
 const logo=box(side,"Mo wordmark","HORIZONTAL",180,0,12);logo.counterAxisAlignItems="CENTER";await txt(logo,"Brand glyph","墨","Display","Paper","accent");await txt(logo,"Brand name","Mo","Heading");
 await txt(side,"Brand line","让表达更自然","Caption","Paper","muted");
 const nav=box(side,"Primary navigation","VERTICAL",180,0,8);nav.paddingTop=24;
 for(const [label,symbol,target] of [["常用设置","输","General"],["外观与皮肤","色","Appearance"],["隐私与学习","隐","Privacy"]]){
  const item=inst(comps["Nav/"+(target===selected?"Selected":"Default")],nav,{Label:label,Symbol:symbol});navigation.push({id:item.id,target});
 }
 const spacer=box(side,"Flexible spacing","VERTICAL",180,0,0);spacer.layoutSizingVertical="FILL";
 await txt(side,"Version","Mo 墨 · 0.0.13","Caption","Paper","muted");await txt(side,"Secondary footer","轻盈表达 · 自在输入","Caption","Paper","muted");
 const main=box(body,"Main pane","VERTICAL",888,0,0);main.resize(888,716);main.primaryAxisSizingMode="FIXED";
 const content=box(main,"Settings content","VERTICAL",888,24,16);content.resize(888,644);content.primaryAxisSizingMode="FIXED";
 const head=box(content,"Page heading","VERTICAL",840,0,4);await txt(head,"Page title",title,"Heading");await txt(head,"Page subtitle",subtitle,"Caption","Paper","muted");
 const footer=box(main,"Save bar","HORIZONTAL",888,0,8,"Paper","bg");footer.resize(888,72);footer.primaryAxisSizingMode="FIXED";footer.counterAxisAlignItems="CENTER";footer.primaryAxisAlignItems="SPACE_BETWEEN";footer.paddingLeft=24;footer.paddingRight=24;footer.strokes=[paint("Paper","border")];
 const left=box(footer,"Restore and status","HORIZONTAL",null,0,12);left.counterAxisAlignItems="CENTER";await button(left,"恢复默认","Ghost");await txt(left,"Save note","更改后请保存","Caption","Paper","muted");
 const right=box(footer,"Save actions","HORIZONTAL",null,0,12);await button(right,"重新读取","Secondary");const save=await button(right,"保存更改");
 return {w,content,head,save};
}
const first=box(root,"Review row 1","HORIZONTAL",2224,0,24);
const general=await shell(first,"General","常用设置","把常用选项放在手边，让每一次输入都顺手。","General");
const input=await card(general.content,"输入习惯");
await setting(input,"输入方案","全拼或常用双拼方案","Choice","全拼");
await setting(input,"输出文字","按你的阅读习惯选择简体或繁体","Choice","简体中文");
const assist=await card(general.content,"候选与表达");
await setting(assist,"候选注释","在候选旁显示拼音等辅助信息","Toggle","On");
await setting(assist,"Emoji 候选","在合适的时候显示 Emoji","Toggle","On");
await txt(general.content,"Deferred effect hint","输入方案将在当前输入结束后生效。","Caption","Paper","muted",820);
const appearance=await shell(first,"Appearance","外观与皮肤","选一种喜欢的样子，输入依然轻盈。","Appearance");
const themeRow=box(appearance.content,"Settings theme","HORIZONTAL",840,12,0,"Paper","canvas");round(themeRow,12);themeRow.resize(840,56);themeRow.primaryAxisSizingMode="FIXED";themeRow.primaryAxisAlignItems="SPACE_BETWEEN";themeRow.counterAxisAlignItems="CENTER";
await txt(themeRow,"Settings theme title","设置界面主题","Strong");
const themeOptions=box(themeRow,"Theme choices","HORIZONTAL",null,0,4);
for(const [label,style] of [["跟随系统","Secondary"],["浅色","Ghost"],["深色","Ghost"]]){
 const b=await button(themeOptions,label,style);b.resize(92,32);
}
const grid=box(appearance.content,"Skin gallery","VERTICAL",840,0,8);
for(const group of [["Paper","Midnight","Mint"],["Dusk","Bamboo","Custom"]]){
 const row=box(grid,"Skin gallery row","HORIZONTAL",840,0,16);row.primaryAxisAlignItems="SPACE_BETWEEN";
 for(const theme of group){
  if(theme!=="Custom"){
   const item=inst(comps["SkinCard/"+theme],row);
   if(theme==="Paper"){item.strokes=[paint("Paper","accent")];item.strokeWeight=2;}
  }else{
   const item=box(row,"Create custom skin","VERTICAL",252,12,8,"Paper","canvas");item.resize(252,136);item.primaryAxisSizingMode="FIXED";item.primaryAxisAlignItems="CENTER";item.counterAxisAlignItems="CENTER";round(item,12);item.strokes=[paint("Paper","border")];icon(item,"add","Paper",24);await txt(item,"Custom title","自定义皮肤","Strong");await txt(item,"Custom caption","把喜欢的配色带进输入","Caption","Paper","muted");customLinks.push(item.id);
  }
 }
}
const preview=box(appearance.content,"Live candidate preview","HORIZONTAL",840,12,24,"Paper","canvas");round(preview,12);preview.counterAxisAlignItems="CENTER";
const prevText=box(preview,"Preview labels","VERTICAL",220,0,4);await txt(prevText,"Preview title","实时预览","Strong");await txt(prevText,"Preview text","纸白 · 横排候选","Caption","Paper","muted");inst(comps["Bubble/Paper/Horizontal"],preview);
const second=box(root,"Review row 2","HORIZONTAL",2224,0,24);
const privacy=await shell(second,"Privacy","隐私与学习","你可以决定，输入习惯如何被记住。","Privacy");
const protection=box(privacy.content,"Privacy summary","HORIZONTAL",840,24,16,"Paper","soft");round(protection,12);icon(protection,"shield","Paper",32);
const protectText=box(protection,"Privacy summary text","VERTICAL",730,0,4);await txt(protectText,"Privacy summary title","学习，留在本机","Section");await txt(protectText,"Privacy summary body","本地学习帮助候选排序；隐私模式会暂停学习。","Body","Paper","muted",718);
const learning=await card(privacy.content,"学习偏好");
await setting(learning,"本地学习","允许 Mo 记住在本机的选词习惯","Toggle","On");
await setting(learning,"隐私模式","暂停学习，不改变本地学习开关的选择","Toggle","Off");
const explainer=box(privacy.content,"Privacy explanation","VERTICAL",840,20,8,"Paper","canvas");round(explainer,12);
await txt(explainer,"Tip heading","暂时不想被记住？","Strong");await txt(explainer,"Tip detail","打开隐私模式即可暂停学习。关闭后，会按原来的本地学习设置继续。","Body","Paper","muted",796);
const editor=await shell(second,"Editor","自定义皮肤","从你喜欢的颜色开始，打造自己的输入气泡。","Appearance");
const back=await button(editor.head,"‹ 返回皮肤","Ghost");backLinks.push(back.id);
const editorRow=box(editor.content,"Editor workspace","HORIZONTAL",840,0,24);
const fields=box(editorRow,"Skin controls","VERTICAL",324,20,12,"Paper","canvas");round(fields,12);
await txt(fields,"Controls heading","我的薄荷","Section");
for(const [title,value,theme,role] of [["背景颜色","#F0F8F2","Mint","bg"],["强调颜色","#286748","Mint","accent"],["文字颜色","#213D2C","Mint","text"]]){
 const line=box(fields,title,"HORIZONTAL",284,0,8);line.counterAxisAlignItems="CENTER";line.primaryAxisAlignItems="SPACE_BETWEEN";await txt(line,"Label",title,"Body");
 const swatch=box(line,"Color swatch","HORIZONTAL",32,0,0,theme,role);swatch.resize(32,32);swatch.primaryAxisSizingMode="FIXED";round(swatch,8);swatch.strokes=[paint("Paper","border")];await txt(line,"Hex value",value,"Caption","Paper","muted");
}
await txt(fields,"Corner label","圆角     12 px","Body");await txt(fields,"Font label","候选字号     16 px","Body");
const layout=inst(comps.Select,fields,{Value:"竖排候选"});layout.resize(284,40);
const customPreview=box(editorRow,"Custom skin preview","VERTICAL",492,20,20,"Mint","canvas");round(customPreview,12);
await txt(customPreview,"Preview heading","我的薄荷 · 预览","Section","Mint");
const bubble=inst(comps["Bubble/Mint/Vertical"],customPreview);customPreview.counterAxisAlignItems="CENTER";
await txt(customPreview,"Readability hint","装饰留在边缘，候选始终清晰。","Caption","Mint","muted");
const customActions=box(editor.content,"Custom skin actions","HORIZONTAL",840,0,12);await button(customActions,"导入本地皮肤","Secondary");await button(customActions,"保存为新皮肤","Primary");
const planned=box(root,"Design scope notes","VERTICAL",2224,0,8);
await txt(planned,"Scope label","设计提案 · 先保留可靠行为，再扩展外观","Section");
await txt(planned,"Scope text","当前已有输入方案、简繁体、注释、Emoji、本地学习、隐私和明暗主题。皮肤库、自定义色、横排与字号是本次新增设计，尚未实装；设置保存继续采用显式保存和冲突保护。","Body","Paper","muted",2160);
const states=box(root,"Save and reload states","HORIZONTAL",2224,0,24);
for(const [title,detail,action,theme] of [
 ["已保存","设置已保存。输入方案将在当前输入结束后生效。","知道了","Paper"],
 ["设置已更新","另一窗口更新了设置。请重新读取后再保存，当前选择会保留。","重新读取","Dusk"],
 ["暂时无法保存","另一窗口正在保存设置。请稍后重试，当前选择会保留。","稍后重试","Paper"]
]){
 const c=box(states,title,"VERTICAL",725,24,12,theme,"bg");round(c,12);c.strokes=[paint(theme,"border")];await txt(c,"State title",title,"Section",theme);await txt(c,"State description",detail,"Body",theme,"muted",677);await button(c,action,title==="已保存"?"Secondary":"Primary");
}
const prototypeScreenIds={};
for(const [key,id] of Object.entries(screenIds)){
 const source=await figma.getNodeByIdAsync(id);const clone=keep(source.clone());page.appendChild(clone);clone.name="Prototype/"+key;clone.x=2600;clone.y=100+Object.keys(prototypeScreenIds).length*820;prototypeScreenIds[key]=clone.id;
}
for(const [key,id] of Object.entries(prototypeScreenIds)){
 const clone=await figma.getNodeByIdAsync(id);
 for(const n of clone.findAllWithCriteria({types:["INSTANCE"]})){
  if(![STATE.components["Nav/Selected"],STATE.components["Nav/Default"]].includes(n.mainComponent.id)){continue;}
  const prop=Object.keys(n.componentProperties).find(p=>p.startsWith("Label#"));
  const label=n.componentProperties[prop].value;
  const target={"常用设置":"General","外观与皮肤":"Appearance","隐私与学习":"Privacy"}[label];
  if(target&&target!==key){await n.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:prototypeScreenIds[target],navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
 }
 const custom=clone.findOne(n=>n.name==="Create custom skin");
 if(custom){await custom.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:prototypeScreenIds.Editor,navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
 const back=clone.findAllWithCriteria({types:["INSTANCE"]}).find(n=>Object.values(n.componentProperties).some(p=>p.type==="TEXT"&&p.value==="‹ 返回皮肤"));
 if(back){await back.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:prototypeScreenIds.Appearance,navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
}
for(const nav of navigation){const n=await figma.getNodeByIdAsync(nav.id);await n.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:prototypeScreenIds[nav.target],navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
for(const id of customLinks){const n=await figma.getNodeByIdAsync(id);await n.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:prototypeScreenIds.Editor,navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
for(const id of backLinks){const n=await figma.getNodeByIdAsync(id);await n.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:prototypeScreenIds.Appearance,navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
figma.root.insertChild(0,page);
const all=root.findAll(()=>true);const counts={};for(const n of all){counts[n.type]=(counts[n.type]||0)+1;}
return {createdNodeIds:allMadeIds(),mutatedNodeIds:[root.id,page.id],screenIds,prototypeScreenIds,navigationCount:navigation.length+customLinks.length+backLinks.length,wrapperBounds:{width:root.width,height:root.height},descendantCount:all.length,nodeTypes:counts,imageFilledNodes:all.filter(n=>Array.isArray(n.fills)&&n.fills.some(p=>p.type==="IMAGE")).map(n=>({id:n.id,name:n.name,type:n.type,width:n.width,height:n.height})),fontFamilies:[...new Set(root.findAllWithCriteria({types:["TEXT"]}).map(n=>n.fontName.family))]};
