import autoprefixer from 'autoprefixer'
import pxtorem from 'postcss-pxtorem'

const config = {
  plugins: [
    // 自动添加浏览器前缀
    autoprefixer({
      overrideBrowserslist: [
        'last 2 versions',
        '> 1%',
        'iOS 7',
        'last 3 iOS versions',
        'Android >= 4.0',
      ],
      flexbox: 'no-2009',
    }),
    // px 转 rem
    pxtorem({
      rootValue: 100, // 根元素字体大小，与HTML中的设置保持一致
      unitPrecision: 5, // 转换后的小数点位数
      propList: ['*'], // 需要转换的属性，*表示所有属性
      selectorBlackList: [
        '.no-rem', // 不转换的类名
      ],
      replace: true, // 是否替换原来的值
      mediaQuery: false, // 是否转换媒体查询中的 px
      minPixelValue: 1, // 小于这个值的 px 不转换
      exclude: /node_modules/i, // 排除 node_modules 文件夹
    }),
  ],
}

export default config
