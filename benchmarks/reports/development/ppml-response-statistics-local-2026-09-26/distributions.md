# Ordered distributions and timing adjudication

B=residualrows2; C=responsestats1; S=pinned comparable stock. Times are ms.
These complete pilot/fresh blocks have clean monitors. Scientific discrepancies are unranked.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot/0; strong_d6; 983040/6/8; low|953;934;944|923;914;918|857;875;864|1.0446–1.0770|
|pilot/1; strong_d6; 983040/6/8; fast|864;870;866|848;844;842|864;867;874|0.9634–0.9815|
|pilot/2; strong_d6; 30720/6/4; low|71;75;70|71;70;70|121;120;120|0.5833–0.5868|
|pilot/3; three_fe_dense; 61440/10/8; fast|149;147;146|153;153;154|231;231;229|0.6623–0.6725|
|pilot/4; three_fe_dense; 153600/19/8; fast|570;560;562|578;561;553|695;713;701|0.7868–0.8317|
|pilot/5; strong_d6; 245760/6/1; fast|541;539;539|533;540;534|936;937;937|0.5694–0.5763|

B/C paired outcomes: {'wins': 10, 'losses': 6, 'ties': 2}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|652;661;658;656;657|656;646;648;650;641|757;770;753;766;762|0.8390–0.8666|
|fresh/1; strong_d6; 737280/6/8; low|711;711;712;714;733|725;695;698;698;719|762;766;769;765;758|0.9073–0.9514|
|fresh/2; strong_d6; 368640/6/4; fast|416;414;413;414;415|408;408;411;408;408|665;674;675;662;654|0.6053–0.6239|
|fresh/3; three_fe_dense; 184320/19/8; fast|676;668;660;665;659|658;670;671;669;663|3127;3137;3227;3202;3246|0.2043–0.2136|
|fresh/4; three_fe_dense; 76800/10/8; fast|192;190;187;189;194|195;185;195;197;189|300;302;300;303;297|0.6126–0.6502|
|fresh/5; three_fe_dense; 38400/6/8; low|117;114;113;117;114|116;114;116;113;119|118;116;119;116;116|0.9741–1.0259|
|fresh/6; strong_d6; 245760/6/1; fast|536;543;543;539;540|540;539;535;533;536|905;937;927;934;938|0.5707–0.5967|
|fresh/7; weak_d3; 61440/6/4; fast|131;131;131;133;135|132;132;131;129;129|777;774;779;777;779|0.1656–0.1705|
|fresh/8; strong_d6; 61440/6/4; low|95;93;93;93;93|93;92;91;90;91|98;98;99;99;99|0.9091–0.9490|

B/C paired outcomes: {'wins': 29, 'losses': 14, 'ties': 2}. Families with every comparable pair <=0.90: 6.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|69;63;64|63;63;66|
|1|75;75;76|74;75;79|
|2|14;11;14|12;15;14|
|3|213;208;214|207;207;212|
|4|159;161;159|164;162;157|
|5|18;18;16|16;18;16|
