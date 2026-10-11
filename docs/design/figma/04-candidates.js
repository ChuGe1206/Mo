const page=await figma.getNodeByIdAsync(STATE.pages.candidates);await figma.setCurrentPageAsync(page);
const root=await figma.getNodeByIdAsync(STATE.wrappers.candidates);
if(root.children.length){throw new Error("Candidate wrapper already populated");}
const comps={};for(const [name,id] of Object.entries(STATE.components)){if(name.startsWith("Bubble/")){comps[name]=await figma.getNodeByIdAsync(id);}}
await txt(root,"Candidate design heading","轻一点，也好看一点。","Display");
await txt(root,"Candidate design caption","紧凑的输入气泡 · 清晰的序号和首选词 · 原创皮肤与系统高对比度","Body","Paper","muted");
const captions={
 Paper:["纸白","默认风格 · 留白、细描边、轻阴影"],
 Midnight:["夜墨","深色环境 · 柔和亮字与低饱和高亮"],
 Mint:["薄荷","清爽绿调 · 清晰、亲近、不抢视线"],
 Dusk:["暮霞","暖色皮肤 · 让长时间输入更轻松"],
 Bamboo:["墨竹","东方绿调 · 原创竹叶装饰只留在边缘"],
 Contrast:["系统高对比度","示意状态 · 实际颜色由 Windows 系统决定"]
};
const tileIds={};
for(const group of [["Paper","Midnight"],["Mint","Dusk"],["Bamboo","Contrast"]]){
 const row=box(root,"Candidate theme row","HORIZONTAL",1280,0,24);
 for(const theme of group){
  const tile=box(row,"Candidate / "+theme,"VERTICAL",628,24,12,"Paper","bg");round(tile,16);tile.strokes=[paint("Paper","border")];tileIds[theme]=tile.id;
  await txt(tile,"Skin heading",captions[theme][0],"Section");
  await txt(tile,"Skin rationale",captions[theme][1],"Caption","Paper","muted");
  const stage=box(tile,"Typing context","VERTICAL",580,24,16,theme,"canvas");round(stage,12);stage.counterAxisAlignItems="CENTER";
  await txt(stage,"Synthetic host text","记下今天的灵感，春风正好。","Body",theme,"muted");
  inst(comps["Bubble/"+theme+"/Horizontal"],stage);
 }
}
const layout=box(root,"Vertical compatibility specimen","HORIZONTAL",1280,24,32,"Paper","bg");round(layout,16);layout.counterAxisAlignItems="CENTER";layout.strokes=[paint("Paper","border")];
inst(comps["Bubble/Paper/Vertical"],layout);
const spec=box(layout,"Layout guidance","VERTICAL",820,0,12);
await txt(spec,"Vertical title","竖排也一样清楚。","Heading");
await txt(spec,"Vertical description","保留完整候选与注释。长词、长句和辅助信息较多时，竖排能更从容地展示。","Body","Paper","muted",784);
await txt(spec,"Interaction rule","数字选词 · 空格确认 · 鼠标选择 · 页码常驻","Strong");
await txt(spec,"Accessibility rule","高对比度优先于皮肤。输入气泡不抢焦点，皮肤装饰不覆盖候选文字。","Body","Paper","muted",784);
await txt(spec,"Synthetic rule","画板内容为合成示例；横排、新皮肤和字号控制属于设计提案。","Caption","Paper","muted",784);
const all=root.findAll(()=>true),types={};for(const n of all){types[n.type]=(types[n.type]||0)+1;}
figma.viewport.scrollAndZoomIntoView([root]);
return {createdNodeIds:allMadeIds(),mutatedNodeIds:[root.id,page.id],tileIds,wrapperBounds:{width:root.width,height:root.height},descendantCount:all.length,nodeTypes:types,imageFilledNodes:all.filter(n=>Array.isArray(n.fills)&&n.fills.some(p=>p.type==="IMAGE")).map(n=>({id:n.id,name:n.name,type:n.type,width:n.width,height:n.height})),fontFamilies:[...new Set(root.findAllWithCriteria({types:["TEXT"]}).map(n=>n.fontName.family))]};
