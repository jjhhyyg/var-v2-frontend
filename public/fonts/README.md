# 中文字体配置说明

## 概述

为了在PDF报告中正确显示中文，需要添加中文字体文件。

## 步骤

### 1. 下载中文字体

推荐使用以下开源字体之一：

- **思源黑体 (Noto Sans SC)** - 推荐
  - 下载地址: <https://github.com/googlefonts/noto-cjk/releases>
  - 文件名: `NotoSansSC-Regular.ttf` 或 `NotoSansCJK-Regular.ttc`

- **思源宋体 (Noto Serif SC)**
  - 下载地址: <https://github.com/googlefonts/noto-cjk/releases>

- **文泉驿微米黑**
  - 下载地址: <http://wenq.org/wqy2/index.cgi?MicroHei>

### 2. 放置字体文件

将下载的TTF字体文件放置在此目录下：

```text
frontend/public/fonts/NotoSansSC-Regular.ttf
```

### 3. 字体子集化（可选，推荐）

完整的中文字体文件通常很大（10-15MB），建议进行字体子集化以减小文件大小。

#### 使用 fonttools 进行字体子集化

```bash
# 安装 fonttools
pip install fonttools brotli

# 提取常用汉字（基本汉字 + 常用标点符号）
pyftsubset NotoSansSC-Regular.ttf \
  --output-file=NotoSansSC-Subset.ttf \
  --unicodes=U+4E00-9FFF,U+3000-303F,U+FF00-FFEF \
  --layout-features='*' \
  --flavor=woff2
```

#### 在线工具

也可以使用在线工具进行字体子集化：

- <https://transfonter.org/> (支持字体子集化)
- <https://everythingfonts.com/subsetter>

### 4. 更新配置（如果使用了不同的字体名称）

如果使用了不同名称的字体文件，需要修改配置文件：

编辑 `frontend/app/utils/chineseFont.ts`:

```typescript
export const CHINESE_FONT_CONFIG = {
  fontFileName: 'YourFont.ttf',  // 修改为你的字体文件名
  fontName: 'YourFont',          // 修改为字体名称
  fontStyle: 'normal' as const,
  fontPath: '/fonts/YourFont.ttf' // 修改为字体路径
}
```

## 字体许可证

使用字体时请遵守相应的许可证：

- **思源黑体/思源宋体**: SIL Open Font License 1.1 (可商用)
- **文泉驿微米黑**: GPL v3 with font embedding exception

## 故障排除

### 问题：PDF 中文字显示为空白或乱码

1. 确认字体文件已正确放置在 `public/fonts/` 目录
2. 检查浏览器控制台是否有字体加载错误
3. 确认字体文件格式为 TTF（不支持 TTC 格式）
4. 清除浏览器缓存后重试

### 问题：字体文件过大

使用字体子集化工具提取常用汉字，可将文件大小从 10MB+ 减少到 2-3MB。

### 问题：某些字符无法显示

检查字体子集化时是否包含了所需的 Unicode 范围，可以扩大 Unicode 范围：

```bash
# 包含更全面的中文字符
pyftsubset NotoSansSC-Regular.ttf \
  --output-file=NotoSansSC-Full.ttf \
  --unicodes=U+0000-FFFF
```

## 参考资料

- [jsPDF 官方文档 - 自定义字体](https://github.com/parallax/jsPDF#use-of-unicode-characters--utf-8)
- [思源黑体 GitHub](https://github.com/googlefonts/noto-cjk)
- [fonttools 文档](https://fonttools.readthedocs.io/)
