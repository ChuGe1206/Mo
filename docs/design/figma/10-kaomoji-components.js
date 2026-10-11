const page=await figma.getNodeByIdAsync(STATE.pages.components);await figma.setCurrentPageAsync(page);await figma.loadFontAsync({family:"Noto Emoji",style:"Regular"});
const family=await figma.getNodeByIdAsync("65:1942");const variants={};const removed=[];
for(const theme of Object.keys(STATE.palettes)){
 const source=await figma.getNodeByIdAsync(V2.bubbles[theme+"/Emoji"]);const c=keep(source.clone());family.appendChild(c);c.name="Skin="+theme+", View=Kaomoji";c.description="独立颜文字面板，与联想表情开关分开。";
 const categories=c.findOne(n=>n.name==="Categories");for(const n of categories.findAllWithCriteria({types:["TEXT"]})){n.fills=[paint(theme,n.name==="Category/颜文字"?"accent":"muted")];}
 const grid=c.findOne(n=>n.name==="Emoji grid");for(const n of [...grid.children]){removed.push(n.id);n.remove();}
 for(const [face,label] of [["(◕‿◕)","开心"],["(づ｡◕‿‿◕｡)づ","拥抱"],["(╯°□°）╯︵ ┻━┻","掀桌"],["( •̀ ω •́ )✧","加油"]]){
  const row=box(grid,"Kaomoji/"+label,"HORIZONTAL",536,8,16,theme,"soft");round(row,8);await txt(row,"Face",face,"Candidate",theme);await txt(row,"Meaning",label,"Caption",theme,"muted");
 }
 variants[theme+"/Kaomoji"]=c.id;
}
let maxY=0;for(let i=0;i<family.children.length;i++){const c=family.children[i];c.x=24+(i%4)*584;c.y=24+Math.floor(i/4)*430;maxY=Math.max(maxY,c.y+c.height);}family.resizeWithoutConstraints(2360,maxY+24);
return {createdNodeIds:allMadeIds().join(","),mutatedNodeIds:[family.id,...family.children.map(n=>n.id)],removedNodeIds:removed,kaomojiVariants:variants};
