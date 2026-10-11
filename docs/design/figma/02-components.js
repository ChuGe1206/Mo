const page=await figma.getNodeByIdAsync(STATE.pages.components);
await figma.setCurrentPageAsync(page);
if(page.findAllWithCriteria({types:["COMPONENT"]}).length){throw new Error("Component page already contains construction");}
const components={};const families={};
for(const style of ["Primary","Secondary","Ghost","Disabled"]){
 const c=keep(figma.createComponent());c.name="Style="+style;c.layoutMode="HORIZONTAL";c.resize(120,40);c.primaryAxisAlignItems="CENTER";c.counterAxisAlignItems="CENTER";gap(c,8);round(c,8);page.appendChild(c);
 fill(c,"Paper",style==="Primary"?"accent":style==="Disabled"?"soft":"bg");
 if(style==="Secondary"){c.strokes=[paint("Paper","border")];c.strokeWeight=1;}
 const text=await txt(c,"Label","保存更改","Strong","Paper",style==="Primary"?"onAccent":style==="Disabled"?"muted":"text");expose(c,text,"Label","保存更改");
 components["Button/"+style]=c.id;
}
families.Button=combine(Object.entries(components).filter(([k])=>k.startsWith("Button/")).map(([,id])=>made.find(n=>n.id===id)),page,"Mo/Button",2,80,80).id;
const toggles=[];
for(const state of ["On","Off"]){
 const c=keep(figma.createComponent());c.name="State="+state;c.layoutMode="HORIZONTAL";c.resize(44,32);c.primaryAxisAlignItems="CENTER";c.counterAxisAlignItems="CENTER";c.fills=[];page.appendChild(c);
 const track=box(c,"Track","HORIZONTAL",44,0,0,"Paper",state==="On"?"accent":"border");track.resize(44,24);track.primaryAxisSizingMode="FIXED";track.counterAxisAlignItems="CENTER";track.primaryAxisAlignItems=state==="On"?"MAX":"MIN";track.paddingLeft=3;track.paddingRight=3;round(track,999);
 const knob=keep(figma.createEllipse());knob.resize(18,18);fill(knob,"Paper","bg");track.appendChild(knob);components["Toggle/"+state]=c.id;toggles.push(c);
}
families.Toggle=combine(toggles,page,"Mo/Toggle",2,480,80).id;
const selects=keep(figma.createComponent());selects.name="Mo/Select";selects.layoutMode="HORIZONTAL";selects.resize(196,40);selects.primaryAxisAlignItems="SPACE_BETWEEN";selects.counterAxisAlignItems="CENTER";selects.paddingLeft=12;selects.paddingRight=12;fill(selects,"Paper","bg");selects.strokes=[paint("Paper","border")];round(selects,8);page.appendChild(selects);selects.x=480;selects.y=210;
expose(selects,await txt(selects,"Value","全拼"),"Value","全拼");icon(selects,"chevron","Paper",16);components.Select=selects.id;
const navs=[];
for(const state of ["Selected","Default"]){
 const c=keep(figma.createComponent());c.name="State="+state;c.layoutMode="HORIZONTAL";c.resize(180,44);c.counterAxisAlignItems="CENTER";c.paddingLeft=12;gap(c,12);round(c,8);fill(c,"Paper",state==="Selected"?"soft":"canvas");page.appendChild(c);
 const mark=box(c,"Mark","HORIZONTAL",20,0,0);
 expose(c,await txt(mark,"Symbol","输","Strong","Paper",state==="Selected"?"accent":"muted"),"Symbol","输");
 expose(c,await txt(c,"Label","常用设置","Strong","Paper",state==="Selected"?"accent":"text"),"Label","常用设置");
 components["Nav/"+state]=c.id;navs.push(c);
}
families.Nav=combine(navs,page,"Mo/Navigation item",2,780,80).id;
for(const type of ["Choice","Toggle"]){
 const c=keep(figma.createComponent());c.name="Mo/Setting row/"+type;c.layoutMode="HORIZONTAL";c.resize(776,72);c.primaryAxisAlignItems="SPACE_BETWEEN";c.counterAxisAlignItems="CENTER";c.fills=[];page.appendChild(c);c.x=80;c.y=400+(type==="Toggle"?100:0);
 const label=box(c,"Text","VERTICAL",520,0,0);
 expose(c,await txt(label,"Title","输入方案","Strong"),"Title","输入方案");
 expose(c,await txt(label,"Description","选择你习惯的拼音方式","Caption","Paper","muted"),"Description","选择你习惯的拼音方式");
 const control=type==="Choice"?inst(selects,c):inst(toggles[0],c);
 components["Row/"+type]=c.id;
}
const bubbles=[];
for(const theme of Object.keys(STATE.palettes)){
 for(const layout of ["Horizontal","Vertical"]){
  const c=keep(figma.createComponent());c.name="Skin="+theme+", Layout="+layout;c.layoutMode="VERTICAL";
  c.resize(layout==="Horizontal"?520:340,1);c.primaryAxisSizingMode="AUTO";c.counterAxisSizingMode="FIXED";pad(c,12);gap(c,8);round(c,12);fill(c,theme,"bg");c.strokes=[paint(theme,"border")];c.strokeWeight=theme==="Contrast"?2:1;await c.setEffectStyleIdAsync(STATE.effects.Bubble);page.appendChild(c);
  const top=box(c,"Preedit","HORIZONTAL",layout==="Horizontal"?496:316,0,0);top.primaryAxisAlignItems="SPACE_BETWEEN";top.counterAxisAlignItems="CENTER";
  expose(c,await txt(top,"Composition","chun feng","Body",theme),"Composition","chun feng");
  await txt(top,"Status","中 · "+(theme==="Contrast"?"高对比度":"Mo"),"Caption",theme,"muted");
  const list=box(c,"Candidates",layout==="Horizontal"?"HORIZONTAL":"VERTICAL",layout==="Horizontal"?496:316,0,layout==="Horizontal"?4:4);
  const words=["春风","春峰","淳风","春丰","春锋"];
  for(let i=0;i<words.length;i++){
   const row=box(list,"Candidate "+(i+1),"HORIZONTAL",layout==="Horizontal"?96:316,0,8,theme,i===0?"accent":"bg");
   row.counterAxisAlignItems="CENTER";row.primaryAxisAlignItems=layout==="Horizontal"?"CENTER":"MIN";row.resize(layout==="Horizontal"?96:316,34);row.primaryAxisSizingMode="FIXED";round(row,8);
   if(layout==="Vertical"){row.paddingLeft=12;}
   await txt(row,"Ordinal",String(i+1),"Caption",theme,i===0?"onAccent":"muted");
   await txt(row,"Candidate",words[i],"Candidate",theme,i===0?"onAccent":"text");
   if(layout==="Vertical"){await txt(row,"Comment",i===0?"chūn fēng":"拼音","Caption",theme,i===0?"onAccent":"muted");}
  }
  const footer=box(c,"Footer","HORIZONTAL",layout==="Horizontal"?496:316,0,0);footer.primaryAxisAlignItems="SPACE_BETWEEN";footer.counterAxisAlignItems="CENTER";
  await txt(footer,"Hint",layout==="Vertical"?"数字选词 · 空格确认":"空格确认","Caption",theme,"muted");
  await txt(footer,"Paging","1 / 3   ‹  ›","Caption",theme,"muted");
  c.description="Mo 原创皮肤候选窗。合成候选内容；高对比度为系统覆盖状态。";
  components["Bubble/"+theme+"/"+layout]=c.id;bubbles.push(c);
 }
}
families.Bubble=combine(bubbles,page,"Mo/Candidate bubble",2,80,780).id;
const skinCards=[];
for(const theme of ["Paper","Midnight","Mint","Dusk","Bamboo"]){
 const c=keep(figma.createComponent());c.name="Skin="+theme;c.layoutMode="VERTICAL";c.resize(252,1);c.primaryAxisSizingMode="AUTO";c.counterAxisSizingMode="FIXED";pad(c,12);gap(c,8);round(c,12);fill(c,"Paper","bg");c.strokes=[paint("Paper","border")];page.appendChild(c);
 const preview=box(c,"Skin preview","VERTICAL",228,12,4,theme,"bg");round(preview,8);
 await txt(preview,"Sample preedit","chun feng","Caption",theme,"muted");
 const candidates=box(preview,"Mini candidates","HORIZONTAL",204,0,8);
 const choice=box(candidates,"Selected","HORIZONTAL",90,4,4,theme,"accent");round(choice,8);choice.counterAxisAlignItems="CENTER";
 await txt(choice,"Selected label","1 春风","Strong",theme,"onAccent");await txt(candidates,"Others","2 春峰","Body",theme);
 const captions={Paper:["纸白","干净、轻盈"],Midnight:["夜墨","深色、专注"],Mint:["薄荷","清爽、柔和"],Dusk:["暮霞","温暖、细腻"],Bamboo:["墨竹","东方、留白"]};
 const caption=box(c,"Caption","HORIZONTAL",228,0,0);caption.primaryAxisAlignItems="SPACE_BETWEEN";
 await txt(caption,"Skin name",captions[theme][0],"Strong");await txt(caption,"Skin description",captions[theme][1],"Caption","Paper","muted");
 components["SkinCard/"+theme]=c.id;skinCards.push(c);
}
families.SkinCard=combine(skinCards,page,"Mo/Skin card",3,1320,780).id;
return {createdNodeIds:allMadeIds(),components,families,componentCount:Object.keys(components).length,counts:{bubbleVariants:bubbles.length,skinCards:skinCards.length},variantFamilies:Object.keys(families),fontFamilies:[...new Set(page.findAllWithCriteria({types:["TEXT"]}).map(n=>n.fontName.family))]};
