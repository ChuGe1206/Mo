const page=await figma.getNodeByIdAsync(STATE.pages.components);await figma.setCurrentPageAsync(page);
if(page.children.some(n=>n.name==="Mo/Candidate bubble v2"))throw new Error("v2 already exists; inspect ledger");
await figma.loadFontAsync({family:"Noto Emoji",style:"Regular"});
const emojiStyle=figma.createTextStyle();emojiStyle.name="Mo / Emoji";emojiStyle.fontName={family:"Noto Emoji",style:"Regular"};emojiStyle.fontSize=26;emojiStyle.lineHeight={unit:"PIXELS",value:36};
const geometry=await figma.variables.getVariableByIdAsync(STATE.variables["spacing/24"]);
const hit=figma.variables.createVariable("size/candidate-tool",geometry.variableCollectionId,"FLOAT");hit.scopes=["WIDTH_HEIGHT"];const coll=await figma.variables.getVariableCollectionByIdAsync(geometry.variableCollectionId);hit.setValueForMode(coll.defaultModeId,28);hit.setVariableCodeSyntax("WEB","var(--mo-candidate-tool-size)");
const smile=await figma.getNodeByIdAsync("58:393");
const iconComponents={Smile:smile.id};
for(const [name,path] of Object.entries({Previous:"m14 6-6 6 6 6",Next:"m9 6 6 6-6 6",Expand:"m6 9 6 6 6-6",Collapse:"m6 15 6-6 6 6",Close:"m6 6 12 12 M18 6 6 18"})){
 const c=keep(figma.createComponent());c.name="Mo/Input icon/"+name;c.resize(20,20);c.fills=[];page.appendChild(c);c.x=2400+Object.keys(iconComponents).length*44;c.y=80;
 const v=keep(figma.createNodeFromSvg('<svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="'+path+'" stroke="#626F68" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>'));c.appendChild(v);for(const n of v.findAllWithCriteria({types:["VECTOR"]})){n.strokes=[paint("Paper","muted")];}iconComponents[name]=c.id;
}
const tools={},toolList=[];
for(const theme of Object.keys(STATE.palettes))for(const state of ["Ready","Disabled"]){
 const c=keep(figma.createComponent());c.name="Skin="+theme+", State="+state;c.layoutMode="HORIZONTAL";c.resize(28,28);c.setBoundVariable("width",hit);c.setBoundVariable("height",hit);c.counterAxisAlignItems="CENTER";c.primaryAxisAlignItems="CENTER";round(c,8);fill(c,theme,"bg");c.opacity=state==="Disabled"?0.35:1;page.appendChild(c);
 const pic=inst(smile,c);pic.name="Icon";for(const v of pic.findAllWithCriteria({types:["VECTOR"]})){if(Array.isArray(v.strokes)&&v.strokes.length)v.strokes=[paint(theme,"muted")];if(Array.isArray(v.fills)&&v.fills.length)v.fills=[paint(theme,"muted")];}
 const prop=c.addComponentProperty("Icon","INSTANCE_SWAP",smile.id);pic.componentPropertyReferences={mainComponent:prop};c.description="28px 命中区；视觉图标20px。禁用态不触发翻页。";tools[theme+"/"+state]=c;toolList.push(c);
}
const toolFamily=combine(toolList,page,"Mo/Candidate tools v2",4,2400,160);
function tool(parent,theme,name,disabled=false){
 const n=inst(tools[theme+"/"+(disabled?"Disabled":"Ready")],parent,{Icon:iconComponents[name]});n.name="Action/"+name;for(const v of n.findAllWithCriteria({types:["VECTOR"]})){if(Array.isArray(v.strokes)&&v.strokes.length)v.strokes=[paint(theme,"muted")];if(Array.isArray(v.fills)&&v.fills.length)v.fills=[paint(theme,"muted")];}return n;
}
const bubbles={},states=["Compact","Expanded","Emoji"],bubbleList=[];
const words=["清","请","情","晴","青","庆"];
const emojiValues=["😊","😂","🥰","😍","😎","🤔","😅","😭","👍","👏","🙏","❤️","🎉","✨","🌸","🌿","😀","😉","😋","🥳","🤗","😴","🙃","😮"];
async function emojiText(parent,value,theme="Paper"){
 const n=keep(figma.createText());n.name="Emoji/"+value;await n.setTextStyleIdAsync(emojiStyle.id);n.characters=value;n.textAutoResize="WIDTH_AND_HEIGHT";n.fills=[paint(theme,"text")];parent.appendChild(n);return n;
}
const cells={};for(const theme of Object.keys(STATE.palettes)){
 const c=keep(figma.createComponent());c.name="Mo/Emoji cell/"+theme;c.layoutMode="HORIZONTAL";c.resize(60,44);c.counterAxisAlignItems="CENTER";c.primaryAxisAlignItems="CENTER";fill(c,theme,"bg");round(c,8);page.appendChild(c);c.x=2400+Object.keys(cells).length*76;c.y=720;expose(c,await emojiText(c,"😊",theme),"Emoji","😊");cells[theme]=c;
}
for(const theme of Object.keys(STATE.palettes))for(const state of states){
 const c=keep(figma.createComponent());c.name="Skin="+theme+", View="+state;c.layoutMode="VERTICAL";c.resize(560,1);c.primaryAxisSizingMode="AUTO";c.counterAxisSizingMode="FIXED";pad(c,12);gap(c,4);round(c,12);fill(c,theme,"bg");c.strokes=[paint(theme,"border")];await c.setEffectStyleIdAsync(STATE.effects.Bubble);page.appendChild(c);
 const top=box(c,"Preedit","HORIZONTAL",536,0,0);top.primaryAxisAlignItems="SPACE_BETWEEN";top.counterAxisAlignItems="CENTER";expose(c,await txt(top,"Composition","qing","Body",theme),"Composition","qing");await txt(top,"Status","中 · 全拼","Caption",theme,"muted");
 const line=box(c,"Candidate line","HORIZONTAL",536,0,8);line.counterAxisAlignItems="CENTER";line.primaryAxisAlignItems="SPACE_BETWEEN";
 const choices=box(line,"Candidates","HORIZONTAL",400,0,4);choices.counterAxisAlignItems="CENTER";
 for(let i=0;i<words.length;i++){
  const row=box(choices,"Candidate "+(i+1),"HORIZONTAL",60,0,4,theme,i===0?"accent":"bg");row.resize(60,32);row.primaryAxisSizingMode="FIXED";row.counterAxisSizingMode="FIXED";row.counterAxisAlignItems="CENTER";row.primaryAxisAlignItems="CENTER";round(row,8);
  await txt(row,"Ordinal",String(i+1),"Caption",theme,i===0?"onAccent":"muted");expose(c,await txt(row,"Candidate",words[i],"Candidate",theme,i===0?"onAccent":"text"),"Candidate"+(i+1),words[i]);
 }
 const actions=box(line,"Actions","HORIZONTAL",124,0,4);actions.counterAxisAlignItems="CENTER";
 tool(actions,theme,"Previous",true);tool(actions,theme,"Next");tool(actions,theme,state==="Expanded"?"Collapse":"Expand");tool(actions,theme,"Smile");
 if(state==="Expanded"){
  const heading=box(c,"More candidates heading","HORIZONTAL",536,0,0);heading.primaryAxisAlignItems="SPACE_BETWEEN";await txt(heading,"Title","更多候选","Strong",theme);await txt(heading,"Help","按需展开 · 长词保留注释","Caption",theme,"muted");
  const extras=["亲情","晴天","清风","青山","青春","清晨","情绪","请问","倾听","清晰","轻盈","庆祝"];
  const grid=box(c,"More candidates grid","VERTICAL",536,0,4);
  for(let j=0;j<4;j++){const line=box(grid,"More row "+j,"HORIZONTAL",536,0,8);for(let k=0;k<3;k++){
   const i=j*3+k;const cell=box(line,"Additional candidate "+(i+7),"VERTICAL",173.33,8,0,theme,"bg");round(cell,8);
   await txt(cell,"Label and candidate",String(i+7)+"  "+extras[i],"Candidate",theme);await txt(cell,"Comment",["qīn qíng","qíng tiān","qīng fēng","qīng shān","qīng chūn","qīng chén","qíng xù","qǐng wèn","qīng tīng","qīng xī","qīng yíng","qìng zhù"][i],"Caption",theme,"muted");
  }}
 }
 if(state==="Emoji"){
  const panel=box(c,"Emoji panel","VERTICAL",536,0,8);
  const head=box(panel,"Emoji heading","HORIZONTAL",536,0,0);head.primaryAxisAlignItems="SPACE_BETWEEN";head.counterAxisAlignItems="CENTER";await txt(head,"Title","表情","Strong",theme);tool(head,theme,"Close");
  const categories=box(panel,"Categories and preference","HORIZONTAL",536,0,16);categories.primaryAxisAlignItems="SPACE_BETWEEN";categories.counterAxisAlignItems="CENTER";
  const tabs=box(categories,"Categories","HORIZONTAL",300,0,16);for(const cat of ["常用","笑脸","手势","颜文字"]){await txt(tabs,"Category/"+cat,cat,"Strong",theme,cat==="常用"?"accent":"muted");}
  const preference=box(categories,"Association preference","HORIZONTAL",204,0,8);preference.primaryAxisAlignItems="MAX";preference.counterAxisAlignItems="CENTER";await txt(preference,"Label","联想表情","Caption",theme,"muted");const toggle=await figma.getNodeByIdAsync(STATE.components["Toggle/On"]);inst(toggle,preference).name="Association toggle";
  const grid=box(panel,"Emoji grid","VERTICAL",536,0,4);for(let j=0;j<3;j++){const row=box(grid,"Emoji row "+j,"HORIZONTAL",536,0,8);for(let k=0;k<8;k++){const cell=inst(cells[theme],row,{Emoji:emojiValues[j*8+k]});cell.name="Emoji choice "+(j*8+k);}}
  await txt(panel,"Input hint","点击插入表情 · Esc 返回候选","Caption",theme,"muted");
 }
 c.description="紧凑候选气泡 v2；右侧28px翻页/展开/表情入口，无常驻页脚。示例为合成内容，表情面板与联想开关是交互提案。";bubbles[theme+"/"+state]=c.id;bubbleList.push(c);
}
const bubbleFamily=combine(bubbleList,page,"Mo/Candidate bubble v2",3,2400,880);
return {createdNodeIds:allMadeIds(),emojiStyle:emojiStyle.id,hitVariable:hit.id,toolFamily:toolFamily.id,bubbleFamily:bubbleFamily.id,icons:iconComponents,tools:Object.fromEntries(Object.entries(tools).map(([k,v])=>[k,v.id])),cells:Object.fromEntries(Object.entries(cells).map(([k,v])=>[k,v.id])),bubbles,dimensions:bubbleList.slice(0,3).map(n=>({id:n.id,width:n.width,height:n.height})),componentCount:toolList.length+bubbleList.length+Object.keys(cells).length+5};
