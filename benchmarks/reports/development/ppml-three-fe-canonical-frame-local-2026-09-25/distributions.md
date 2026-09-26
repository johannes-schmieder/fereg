## Pilot distributions

Times in milliseconds, repetition order; B=finiteblock1, C=canonicalthree1.

|Case; design / N / finite K / T / memory|B|C|Stock|C/stock range|
|---|---|---|---|---|
|0: three_fe_dense / 153600 / 19 / 8 / fast; factor_cross; unbalanced; 3 FE; 0 slopes; cluster cl1 cl2|701, 695, 698|590, 602, 587|712, 705, 699|0.829–0.854|
|1: three_fe_dense / 153600 / 19 / 8 / fast; numeric; unbalanced; 3 FE; 0 slopes; cluster cl1 cl2|575, 563, 598|472, 468, 474|615, 621, 615|0.754–0.771|
|2: three_fe_dense / 30720 / 6 / 4 / fast; numeric; unbalanced; 3 FE; 0 slopes; robust|83, 83, 82|76, 83, 78|82, 82, 83|0.927–1.012|
|3: strong_d6 / 983040 / 6 / 8 / fast; numeric; standard; 2 FE; 0 slopes; robust|1227, 1225, 1265|1233, 1329, 1238|864, 866, 859|1.427–1.535|
|4: three_fe_dense / 153600 / 19 / 8 / low; factor_cross; unbalanced; 3 FE; 0 slopes; cluster cl1 cl2|707, 706, 709|522, 523, 523|699, 702, 694|0.745–0.754|
|5: strong_d6 / 30720 / 6 / 8 / low; numeric; standard; 2 FE; 2 slopes; robust|70, 68, 70|72, 70, 69|80, 80, 82|0.841–0.900|
|6: three_fe_dense / 30720 / 6 / 4 / fast; numeric; four_fe; 4 FE; 0 slopes; robust|125, 124, 123|121, 127, 123|320, 321, 319|0.378–0.396|
|7: three_fe_dense / 1920 / 2 / 1 / fast; numeric; standard; 3 FE; 0 slopes; robust|17, 16, 16|17, 15, 17|34, 37, 39|0.405–0.500|
|8: three_fe_dense / 38400 / 8 / 1 / fast; numeric; standard; 3 FE; 0 slopes; robust|188, 191, 185|185, 183, 186|1311, 1314, 1313|0.139–0.142|
|9: three_fe_sparse / 38400 / 8 / 4 / fast; numeric; standard; 3 FE; 0 slopes; robust|178, 179, 183|178, 180, 180|11234, 11178, 11214|unranked: means|
|10: three_fe_dense / 38400 / 6 / 4 / fast; numeric; disconnected; 3 FE; 0 slopes; cluster cl1 cl2|249, 243, 245|249, 248, 245|10041, 10159, 10118|0.024–0.025|
|11: three_fe_dense / 15360 / 65 / 4 / fast; numeric; standard; 3 FE; 0 slopes; robust|226, 225, 223|228, 224, 224|1020, 1005, 1014|0.221–0.224|
|12: three_fe_dense / 153600 / 19 / 8 / low; numeric; unbalanced; 3 FE; 0 slopes; cluster cl1 cl2|652, 658, 652|452, 448, 451|623, 624, 625|0.718–0.726|

## Fresh distributions

Times in milliseconds, repetition order; B=finiteblock1, C=canonicalthree1.

|Case; design / N / finite K / T / memory|B|C|Stock|C/stock range|
|---|---|---|---|---|
|0: three_fe_dense / 184320 / 19 / 8 / fast; factor_cross; unbalanced; 3 FE; 0 slopes; cluster cl1 cl2|895, 882, 870, 864, 877|719, 718, 713, 721, 723|3138, 3156, 3148, 3097, 3172|0.226–0.233|
|1: three_fe_dense / 38400 / 8 / 1 / fast; numeric; standard; 3 FE; 0 slopes; robust|187, 187, 187, 187, 184|188, 186, 186, 187, 183|1296, 1274, 1275, 1278, 1275|0.144–0.146|
|2: three_fe_sparse / 38400 / 8 / 4 / fast; numeric; standard; 3 FE; 0 slopes; robust|191, 184, 185, 185, 185|185, 195, 186, 187, 185|14086, 14055, 14108, 14070, 14054|unranked: means|
|3: three_fe_dense / 38400 / 6 / 4 / fast; numeric; disconnected; 3 FE; 0 slopes; cluster cl1 cl2|193, 187, 188, 187, 188|193, 192, 195, 186, 194|8770, 8758, 8741, 8729, 8819|0.021–0.022|
|4: three_fe_dense / 15360 / 65 / 4 / fast; numeric; standard; 3 FE; 0 slopes; robust|225, 225, 224, 227, 225|223, 227, 225, 222, 225|995, 1017, 1004, 1007, 999|0.220–0.225|
|5: strong_d6 / 737280 / 6 / 8 / fast; numeric; standard; 2 FE; 0 slopes; robust|933, 938, 933, 931, 994|980, 932, 933, 931, 927|774, 766, 751, 758, 762|1.217–1.266|
|6: three_fe_dense / 61440 / 19 / 4 / low; factor_cross; unbalanced; 3 FE; 0 slopes; cluster cl1 cl2|359, 365, 334, 329, 337|284, 286, 287, 304, 308|404, 406, 402, 412, 410|0.703–0.751|
|7: three_fe_dense / 38400 / 6 / 8 / low; numeric; unbalanced; 3 FE; 4 slopes; robust|120, 119, 119, 118, 122|120, 120, 119, 118, 120|117, 119, 114, 119, 119|0.992–1.044|
|8: three_fe_dense / 76800 / 12 / 4 / fast; numeric; unbalanced; 3 FE; 0 slopes; robust|321, 314, 315, 321, 320|304, 307, 304, 306, 304|354, 349, 349, 349, 353|0.859–0.880|
|9: strong_d6 / 153600 / 19 / 8 / fast; factor_cross; standard; 2 FE; 0 slopes; robust|423, 421, 414, 423, 421|417, 423, 418, 425, 416|507, 509, 501, 508, 524|0.794–0.837|

