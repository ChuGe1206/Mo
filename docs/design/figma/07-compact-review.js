await figma.loadFontAsync({family:'Noto Emoji',style:'Regular'});

const page=await figma.getNodeByIdAsync(STATE.pages.candidates);await figma.setCurrentPageAsync(page);const root=await figma.getNodeByIdAsync("66:549");if(root.children.length)throw new Error("review populated");
fill(root,"Paper","canvas");pad(root,40);gap(root,24);
const comps={};for(const [key,id] of Object.entries(V2.bubbles))comps[key]=await figma.getNodeByIdAsync(id);
await txt(root,"Heading","少占空间，多一点表达。","Display");
await txt(root,"Summary","两行输入气泡 · 右侧轻量翻页 · 更多候选按需展开 · 独立表情面板","Body","Paper","muted");
const lead=box(root,"Compact default example","HORIZONTAL",1200,24,24,"Paper","bg");round(lead,16);lead.counterAxisAlignItems="CENTER";
inst(comps["Paper/Compact"],lead);
const details=box(lead,"Compact guidance","VERTICAL",540,0,8);
await txt(details,"Title","翻页只占两个小箭头","Heading");
await txt(details,"Description","移除常驻页脚。首屏保留 6 个候选，右侧集中翻页、展开和表情入口。","Body","Paper","muted",516);
await txt(details,"Dimensions","560 × 86 · 比原横排高度减少 28%","Caption","Paper","muted");
const labels={Paper:"纸白",Midnight:"夜墨",Mint:"薄荷",Dusk:"暮霞",Bamboo:"墨竹",Contrast:"系统高对比度"};
for(const pair of [["Paper","Midnight"],["Mint","Dusk"],["Bamboo","Contrast"]]){
 const row=box(root,"Compact skins","HORIZONTAL",1200,0,24);
 for(const theme of pair){const tile=box(row,"Skin/"+theme,"VERTICAL",588,12,8,"Paper","bg");round(tile,12);await txt(tile,"Label",labels[theme],"Strong");inst(comps[theme+"/Compact"],tile);}
}
const states=box(root,"Expanded and emoji states","HORIZONTAL",1200,0,24);
const expanded=box(states,"Expanded sample","VERTICAL",588,12,12,"Paper","bg");round(expanded,12);await txt(expanded,"Title","更多候选 · 长词与注释","Section");await txt(expanded,"Caption","按需展开；再次点击上箭头收起","Caption","Paper","muted");inst(comps["Paper/Expanded"],expanded);
const emoji=box(states,"Emoji sample","VERTICAL",588,12,12,"Paper","bg");round(emoji,12);await txt(emoji,"Title","独立表情面板","Section");await txt(emoji,"Caption","分类浏览与中文联想各自控制","Caption","Paper","muted");inst(comps["Paper/Emoji"],emoji);
const extras=box(root,"Emoji options","HORIZONTAL",1200,0,24);
const kaomoji=box(extras,"Kaomoji sample","VERTICAL",588,12,12,"Paper","bg");round(kaomoji,12);await txt(kaomoji,"Title","颜文字也可以输入","Section");
const ka=inst(comps["Paper/Emoji"],kaomoji);ka.name="Kaomoji example";
const panel=ka.findOne(n=>n.name==="Emoji panel");const categories=ka.findOne(n=>n.name==="Categories");
for(const n of categories.findAllWithCriteria({types:["TEXT"]})){n.fills=[paint("Paper",n.name==="Category/颜文字"?"accent":"muted")];}
const grid=ka.findOne(n=>n.name==="Emoji grid");grid.visible=false;
const faces=box(kaomoji,"Kaomoji choices","VERTICAL",560,12,8,"Paper","bg");round(faces,12);
for(const [face,label] of [["(◕‿◕)","开心"],["(づ｡◕‿‿◕｡)づ","拥抱"],["(╯°□°）╯︵ ┻━┻","掀桌"],["( •̀ ω •́ )✧","加油"]]){const row=box(faces,"Kaomoji/"+label,"HORIZONTAL",536,8,16,"Paper","soft");round(row,8);await txt(row,"Face",face,"Candidate");await txt(row,"Meaning",label,"Caption","Paper","muted");}
const options=box(extras,"Emoji association options","VERTICAL",588,24,16,"Paper","bg");round(options,12);await txt(options,"Title","联想表情，与面板分开","Heading");
const assoc=box(options,"Association setting","HORIZONTAL",540,0,0);assoc.primaryAxisAlignItems="SPACE_BETWEEN";await txt(assoc,"Label","中文联想表情","Strong");inst(await figma.getNodeByIdAsync(STATE.components["Toggle/On"]),assoc);
await txt(options,"Explanation","开启时，中文候选中可出现匹配的 Emoji；关闭后仍可主动打开表情面板。","Body","Paper","muted",540);
const suggestions=box(options,"Association example","HORIZONTAL",540,12,12,"Paper","soft");round(suggestions,8);await txt(suggestions,"Chinese","开心","Candidate");const emot=keep(figma.createText());await emot.setTextStyleIdAsync(V2.emojiStyle);emot.characters="😊";suggestions.appendChild(emot);await txt(suggestions,"Label","联想候选","Caption","Paper","muted");
await txt(options,"Interaction details","悬停显示按钮用途；无可翻页内容时禁用箭头。窄屏优先缩减首屏候选，长词在展开区显示。","Body","Paper","muted",540);
await txt(options,"High contrast","高对比度服从系统；所有面板保持输入焦点，Esc 返回当前候选。","Caption","Paper","muted",540);
const prototypes={};
for(const [name,key] of Object.entries({Compact:"Paper/Compact",Next:"Paper/Compact",Expanded:"Paper/Expanded",Emoji:"Paper/Emoji",EmojiOff:"Paper/Emoji"})){
 const frame=box(page,"Mo/Input prototype/"+name,"VERTICAL",720,40,24,"Paper","canvas");frame.resize(720,640);frame.primaryAxisSizingMode="FIXED";frame.x=root.x+root.width+160+(Object.keys(prototypes).length%2)*800;frame.y=100+Math.floor(Object.keys(prototypes).length/2)*720;
 await txt(frame,"Host sentence","今晚一起散步吧。","Heading");const bubble=inst(comps[key],frame);bubble.name="Interactive bubble";
 if(name==="Next"){const vals=["轻","倾","卿","亲","氢","顷"];const props={};for(let i=0;i<6;i++)props["Candidate"+(i+1)]=vals[i];const keys=Object.keys(bubble.componentProperties);bubble.setProperties(Object.fromEntries(Object.entries(props).map(([k,v])=>[keys.find(x=>x===k||x.startsWith(k+"#")),v])));const prev=bubble.findOne(n=>n.name==="Action/Previous");prev.swapComponent(await figma.getNodeByIdAsync(V2.tools["Paper/Ready"]));const pkey=Object.keys(prev.componentProperties).find(k=>k.startsWith("Icon#"));prev.setProperties({[pkey]:V2.icons.Previous});}
 if(name==="EmojiOff"){const toggle=bubble.findOne(n=>n.name==="Association toggle");toggle.swapComponent(await figma.getNodeByIdAsync(STATE.components["Toggle/Off"]));}
 prototypes[name]={frame:frame.id,bubble:bubble.id};
}
const done=box(page,"Mo/Input prototype/Emoji inserted","VERTICAL",720,40,24,"Paper","canvas");done.resize(720,640);done.primaryAxisSizingMode="FIXED";done.x=root.x+root.width+1760;done.y=100;const sentence=box(done,"Inserted sentence","HORIZONTAL",640,0,4);await txt(sentence,"Text","今晚一起散步吧。","Heading");const mark=keep(figma.createText());await mark.setTextStyleIdAsync(V2.emojiStyle);mark.characters="😊";sentence.appendChild(mark);await txt(done,"Caption","表情已插入当前输入位置","Body","Paper","muted");prototypes.Inserted={frame:done.id};
async function link(node,dest){if(!node)return;await node.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:dest,navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
for(const [name,data] of Object.entries(prototypes)){if(!data.bubble)continue;const b=await figma.getNodeByIdAsync(data.bubble);
 await link(b.findOne(n=>n.name==="Action/Smile"),prototypes[name.startsWith("Emoji")?"Compact":"Emoji"].frame);
 await link(b.findOne(n=>n.name==="Action/Expand"),prototypes.Expanded.frame);
 await link(b.findOne(n=>n.name==="Action/Collapse"),prototypes.Compact.frame);
 await link(b.findOne(n=>n.name==="Action/Close"),prototypes.Compact.frame);
 if(name==="Next")await link(b.findOne(n=>n.name==="Action/Previous"),prototypes.Compact.frame);
 else if(!name.startsWith("Emoji"))await link(b.findOne(n=>n.name==="Action/Next"),prototypes.Next.frame);
 if(name.startsWith("Emoji")){await link(b.findOne(n=>n.name==="Association toggle"),prototypes[name==="Emoji"?"EmojiOff":"Emoji"].frame);await link(b.findOne(n=>n.name==="Emoji choice 1"),prototypes.Inserted.frame);}
}
const all=root.findAll(()=>true),counts={};for(const n of all)counts[n.type]=(counts[n.type]||0)+1;
figma.viewport.scrollAndZoomIntoView([root]);
return {createdNodeIds:allMadeIds().join(','),mutatedNodeIds:[root.id,page.id],reviewId:root.id,prototypes,bounds:{width:root.width,height:root.height},nodeTypes:counts,imageFills:all.filter(n=>Array.isArray(n.fills)&&n.fills.some(p=>p.type==="IMAGE")).length,fontFamilies:[...new Set(root.findAllWithCriteria({types:["TEXT"]}).map(n=>n.fontName.family))]};
