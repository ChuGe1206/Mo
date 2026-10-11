const page=await figma.getNodeByIdAsync("2:3");await figma.setCurrentPageAsync(page);await figma.loadFontAsync({family:"Noto Emoji",style:"Regular"});
const ka=await figma.getNodeByIdAsync("68:3306");ka.swapComponent(await figma.getNodeByIdAsync("75:772"));
const faces=await figma.getNodeByIdAsync("68:3417");const removed=[faces.id,...faces.findAll(()=>true).map(n=>n.id)];faces.remove();
const frame=box(page,"Mo/Input prototype/Kaomoji","VERTICAL",720,40,24,"Paper","canvas");frame.resize(720,640);frame.primaryAxisSizingMode="FIXED";frame.x=4420;frame.y=1540;await txt(frame,"Host sentence","今晚一起散步吧。","Heading");const bubble=inst(await figma.getNodeByIdAsync("75:772"),frame);bubble.name="Interactive bubble";
async function link(n,id){if(n)await n.setReactionsAsync([{trigger:{type:"ON_CLICK"},actions:[{type:"NODE",destinationId:id,navigation:"NAVIGATE",transition:null,resetScrollPosition:true}]}]);}
const mutated=[ka.id];for(const name of ["Emoji","EmojiOff"]){const b=await figma.getNodeByIdAsync(PROTO[name].bubble);const tab=b.findOne(n=>n.name==="Category/颜文字");await link(tab,frame.id);mutated.push(tab.id);}
for(const name of ["Category/常用","Category/笑脸"]){const tab=bubble.findOne(n=>n.name===name);await link(tab,PROTO.Emoji.frame);mutated.push(tab.id);}
for(const name of ["Action/Smile","Action/Close"]){const button=bubble.findOne(n=>n.name===name);await link(button,PROTO.Compact.frame);mutated.push(button.id);}
const all=(await figma.getNodeByIdAsync("66:549")).findAll(()=>true),counts={};for(const n of all)counts[n.type]=(counts[n.type]||0)+1;
return {createdNodeIds:allMadeIds(),mutatedNodeIds:mutated,removedNodeIds:removed,kaomojiPrototype:{frame:frame.id,bubble:bubble.id},nodeTypes:counts,imageFills:all.filter(n=>Array.isArray(n.fills)&&n.fills.some(p=>p.type==="IMAGE")).length,fontFamilies:[...new Set(all.filter(n=>n.type==="TEXT").map(n=>n.fontName.family))]};
