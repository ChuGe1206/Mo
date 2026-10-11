if(figma.root.children.some(p=>p.name==="04 · Mo 应用图标"))throw new Error("icon page exists; inspect ledger");
const page=figma.createPage();page.name="04 · Mo 应用图标";await figma.setCurrentPageAsync(page);
const components={};
for(const [key,svg] of Object.entries(ICONS)){
 const c=keep(figma.createComponent());c.name="Mo/App icon/"+key;c.resize(key==="mo-app-icon"?256:32,key==="mo-app-icon"?256:32);c.fills=[];page.appendChild(c);c.x=1400+Object.keys(components).length*360;c.y=100;
 const art=keep(figma.createNodeFromSvg(svg));art.name="Original editable SVG";c.appendChild(art);c.description="Mo 原创 M 字形与输入光标。Apache-2.0，源文件 assets/icons/"+key+".svg。";
 for(const n of art.findAll(()=>true)){
  for(const field of ["fills","strokes"]){if(!Array.isArray(n[field]))continue;const paints=n[field].map(p=>{if(p.type!=="SOLID")return p;const hex=[p.color.r,p.color.g,p.color.b].map(v=>Math.round(v*255).toString(16).padStart(2,"0")).join("").toUpperCase();const mapping={"176D5E":["Paper","accent"],"FFFFFF":["Paper","onAccent"],"9ADAC5":["Midnight","accent"],"25352F":["Paper","text"]};return mapping[hex]?paint(...mapping[hex]):p;});n[field]=paints;}
 }
 components[key]=c.id;
}
const root=box(page,"Mo · 现代应用图标 / Review","VERTICAL",1100,40,24,"Paper","canvas");root.x=100;root.y=100;
await txt(root,"Title","Mo 墨 · 一笔成形。","Display");
await txt(root,"Subtitle","墨绿底色 · 简洁 M 字形 · 输入光标 · 无字体依赖的原创矢量","Body","Paper","muted");
const hero=box(root,"Application identity","HORIZONTAL",1020,32,32,"Paper","bg");round(hero,20);hero.counterAxisAlignItems="CENTER";
inst(await figma.getNodeByIdAsync(components["mo-app-icon"]),hero);
const desc=box(hero,"Identity rationale","VERTICAL",560,0,16);await txt(desc,"Name","Mo 墨输入法","Heading");
await txt(desc,"Meaning","M 的两笔连接成简洁的输入标记；底部的小光标强调文字输入。墨绿延续纸白气泡的强调色。","Body","Paper","muted",560);
await txt(desc,"Usage","应用启动器 / 安装程序 / 设置窗口","Strong");
await txt(desc,"Format","可编辑 SVG · 透明外角 · Apache-2.0","Caption","Paper","muted");
const sizes=box(root,"Pixel-scale review","VERTICAL",1020,24,16,"Paper","bg");round(sizes,16);await txt(sizes,"Title","小尺寸保持清楚","Section");
const lineup=box(sizes,"Application sizes","HORIZONTAL",972,0,32);lineup.counterAxisAlignItems="MAX";
for(const size of [16,24,32,48,64,128]){const tile=box(lineup,"Size "+size,"VERTICAL",Math.max(72,size),0,12);tile.counterAxisAlignItems="CENTER";const n=inst(await figma.getNodeByIdAsync(components["mo-app-icon"]),tile);n.rescale(size/256);await txt(tile,"Size label",size+" px","Caption","Paper","muted");}
const pair=box(root,"Monochrome marks","HORIZONTAL",1020,0,24);
for(const [key,theme,title] of [["mo-mark-on-light","Paper","浅色背景 · 状态栏标记"],["mo-mark-on-dark","Midnight","深色背景 · 状态栏标记"]]){
 const tile=box(pair,title,"VERTICAL",498,24,16,theme,"bg");round(tile,16);await txt(tile,"Title",title,"Strong",theme);
 const samples=box(tile,"Mark sizes","HORIZONTAL",450,0,32,theme);samples.counterAxisAlignItems="CENTER";
 for(const size of [16,24,32]){const n=inst(await figma.getNodeByIdAsync(components[key]),samples);n.rescale(size/32);}
 await txt(tile,"Note","单色版有透明背景；高对比度采用系统前景色。","Caption",theme,"muted",450);
}
const all=root.findAll(()=>true),counts={};for(const n of all)counts[n.type]=(counts[n.type]||0)+1;figma.viewport.scrollAndZoomIntoView([root]);
return {createdNodeIds:allMadeIds(),pageId:page.id,components,reviewId:root.id,bounds:{width:root.width,height:root.height},nodeTypes:counts,imageFills:all.filter(n=>Array.isArray(n.fills)&&n.fills.some(p=>p.type==="IMAGE")).length,fontFamilies:[...new Set(root.findAllWithCriteria({types:["TEXT"]}).map(n=>n.fontName.family))]};
