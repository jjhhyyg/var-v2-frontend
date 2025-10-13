/**
 * 中文字体工具
 * 用于为 jsPDF 提供中文字体支持
 *
 * 使用方法：
 * 1. 下载中文字体文件（如思源黑体、微软雅黑等）
 * 2. 将字体文件放置在 public/fonts 目录下，并在 CHINESE_FONT_CONFIG 中配置字体文件名和路径
 * 3. 在生成 PDF 时，调用 loadChineseFontFromFile 函数加载字体文件并转换为 base64
 * 4. 使用 jsPDF 的 addFileToVFS 和 addFont 方法注册字体
 * 5. 在生成 PDF 时，设置字体为注册的中文字体
 * 6. 生成的 PDF 即可正确显示中文字符
 */

/**
 * 动态加载中文字体文件
 * 从 public 目录加载字体文件并转换为 base64
 */
export const loadChineseFontFromFile = async (fontPath: string = '/fonts/NotoSansSC-Regular.ttf'): Promise<string> => {
  try {
    const response = await fetch(fontPath)
    if (!response.ok) {
      throw new Error(`Failed to load font: ${response.statusText}`)
    }

    const blob = await response.blob()
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => {
        const base64: string = (reader.result as string).split(',')[1] ?? ''
        resolve(base64)
      }
      reader.onerror = reject
      reader.readAsDataURL(blob)
    })
  } catch (error) {
    console.error('Failed to load Chinese font:', error)
    throw error
  }
}

/**
 * 字体配置
 */
export const CHINESE_FONT_CONFIG = {
  // 字体文件名（在 VFS 中的名称）
  fontFileName: 'NotoSerifSC-VF.ttf',
  // 字体名称（在 PDF 中的名称）
  fontName: 'Noto Serif SC Extra Light',
  // 字体样式
  fontStyle: 'normal' as const,
  // 字体路径（public 目录下的路径）
  fontPath: '/fonts/NotoSerifSC-VF.ttf'
}
