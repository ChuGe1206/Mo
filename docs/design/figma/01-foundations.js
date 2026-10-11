// Mo design exploration; Apache-2.0. Existing product fonts: Microsoft YaHei UI.
// Figma substitutes verified Noto Sans SC; no font binary is included.
const page=await figma.getNodeByIdAsync("0:1");
await figma.setCurrentPageAsync(page);
if(figma.currentPage.children.length){throw new Error("Foundations require the inspected empty file");}
await Promise.all(["Regular","Medium","Bold"].map(style=>figma.loadFontAsync({family:"Noto Sans SC",style})));
page.name="03 · 组件与规范";
const screens=figma.createPage();screens.name="01 · 设置界面";
const candidates=figma.createPage();candidates.name="02 · 候选气泡";
const created=[page.id,screens.id,candidates.id];
async function createVariableCollection(name,modeNames){
  const collection=figma.variables.createVariableCollection(name);
  const modeIds={};const first=collection.modes[0];
  collection.renameMode(first.modeId,modeNames[0]);modeIds[modeNames[0]]=first.modeId;
  for(let i=1;i<modeNames.length;i++){modeIds[modeNames[i]]=collection.addMode(modeNames[i]);}
  return {collection,modeIds};
}
const primitives=await createVariableCollection("Mo / Primitives",["Value"]);
const colors=await createVariableCollection("Mo / Color",["Value"]);
const geometry=await createVariableCollection("Mo / Geometry",["Value"]);
const palettes={
 Paper:{bg:"#FFFFFF",canvas:"#F4F6F5",text:"#25352F",muted:"#626F68",accent:"#176D5E",onAccent:"#FFFFFF",soft:"#EAF3EE",border:"#E0E6E2"},
 Midnight:{bg:"#242B29",canvas:"#171D1B",text:"#EFF5F1",muted:"#B0BEB5",accent:"#9ADAC5",onAccent:"#15352C",soft:"#35463E",border:"#4C5952"},
 Mint:{bg:"#F0F8F2",canvas:"#E3F0E8",text:"#213D2C",muted:"#526A59",accent:"#286748",onAccent:"#FFFFFF",soft:"#D4EADD",border:"#C7DECF"},
 Dusk:{bg:"#FFF4EF",canvas:"#F4E8E0",text:"#4A3029",muted:"#795C50",accent:"#98503F",onAccent:"#FFFFFF",soft:"#F4DCD0",border:"#EBD3C7"},
 Bamboo:{bg:"#F4F4EA",canvas:"#E8ECDF",text:"#303C2A",muted:"#60684F",accent:"#526143",onAccent:"#FFFFFF",soft:"#E2E7D2",border:"#CDD4BA"},
 Contrast:{bg:"#000000",canvas:"#191919",text:"#FFFFFF",muted:"#FFFFFF",accent:"#FFFF00",onAccent:"#000000",soft:"#262626",border:"#FFFFFF"}
};
const primitiveByHex={};const variables={};
const solidScopes=["FRAME_FILL","SHAPE_FILL","TEXT_FILL","STROKE_COLOR"];
for(const [theme,values] of Object.entries(palettes)){
 for(const [role,hex] of Object.entries(values)){
  if(!primitiveByHex[hex]){
   const name="swatch/"+hex.slice(1).toLowerCase();const p=figma.variables.createVariable(name,primitives.collection,"COLOR");
   p.scopes=[];p.setValueForMode(primitives.modeIds.Value,{r:parseInt(hex.slice(1,3),16)/255,g:parseInt(hex.slice(3,5),16)/255,b:parseInt(hex.slice(5,7),16)/255,a:1});
   p.setVariableCodeSyntax("WEB","var(--mo-"+name.replace("/","-")+")");primitiveByHex[hex]=p;
  }
  const name="color/"+theme.toLowerCase()+"/"+role;
  const v=figma.variables.createVariable(name,colors.collection,"COLOR");
  v.scopes=role==="text"||role==="muted"||role==="onAccent"?["TEXT_FILL"]:role==="border"?["STROKE_COLOR"]:solidScopes;
  v.setValueForMode(colors.modeIds.Value,{type:"VARIABLE_ALIAS",id:primitiveByHex[hex].id});
  v.setVariableCodeSyntax("WEB","var(--mo-"+theme.toLowerCase()+"-"+role.replace(/([A-Z])/g,"-$1").toLowerCase()+")");
  variables[name]=v.id;
 }
}
for(const n of [0,4,8,12,16,20,24,32,40]){
 const name="spacing/"+n;const v=figma.variables.createVariable(name,geometry.collection,"FLOAT");v.scopes=["GAP"];v.setValueForMode(geometry.modeIds.Value,n);v.setVariableCodeSyntax("WEB","var(--mo-space-"+n+")");variables[name]=v.id;
}
for(const n of [8,12,16,20,999]){
 const name="radius/"+n;const v=figma.variables.createVariable(name,geometry.collection,"FLOAT");v.scopes=["CORNER_RADIUS"];v.setValueForMode(geometry.modeIds.Value,n);v.setVariableCodeSyntax("WEB","var(--mo-radius-"+n+")");variables[name]=v.id;
}
const styles={};
for(const [name,size,line,weight] of [["Display",32,46,"Bold"],["Heading",24,34,"Medium"],["Section",16,26,"Medium"],["Body",14,24,"Regular"],["Strong",14,24,"Medium"],["Caption",12,20,"Regular"],["Candidate",16,26,"Regular"]]){
 const s=figma.createTextStyle();s.name="Mo / "+name;s.fontName={family:"Noto Sans SC",style:weight};s.fontSize=size;s.lineHeight={unit:"PIXELS",value:line};styles[name]=s.id;
}
const effects={};
for(const [name,y,blur,opacity] of [["Bubble",6,22,0.13],["Window",12,40,0.12]]){
 const s=figma.createEffectStyle();s.name="Mo / "+name;s.effects=[{type:"DROP_SHADOW",color:{r:0.08,g:0.15,b:0.11,a:opacity},offset:{x:0,y},radius:blur,spread:0,visible:true,blendMode:"NORMAL"}];effects[name]=s.id;
}
return {createdNodeIds:created,pages:{components:page.id,screens:screens.id,candidates:candidates.id},variables,styles,effects,palettes,font:{family:"Noto Sans SC",styles:["Regular","Medium","Bold"]},collectionIds:[primitives.collection.id,colors.collection.id,geometry.collection.id],primitiveIds:Object.values(primitiveByHex).map(v=>v.id),semanticVariableCount:Object.keys(variables).length,discovery:{codeConnectFiles:0,existingScreens:0,localAssets:0,libraryChecked:"Simple Design System",decision:"Mo-specific Chinese desktop skin palette and component contract; use local variants",fontFallback:"Microsoft YaHei UI unavailable in Figma; Noto Sans SC verified",freePlanModeStrategy:"one Value mode; skin tokens via variant namespaces"}};
